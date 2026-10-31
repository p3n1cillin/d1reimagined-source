
#include <algorithm>
#include <atomic>
#include <cstdint>
#include <functional>
#include <iostream>
#include <mutex>
#include <string>
#include <thread>
#include <utility>
#include <vector>
using u64=std::uint64_t;
std::atomic<u64> clock_us{0}, file_checks{0}, cap_notices{0};
std::atomic<bool> marker{false};
u64 get_system_time() { return clock_us.load(); }
namespace fs {
bool is_file(const std::string& name) {
    ++file_checks;
    if(name!="private-control/d1descriptor") throw std::runtime_error("wrong marker");
    return marker.load();
}
}
struct diagnostic_logger {
    void notice(const char*,u64 limit) {
        if(limit!=262'144) throw std::runtime_error("wrong cap");
        ++cap_notices;
    }
} d1_log;
namespace d1diag {
const std::string& ctl_dir() { static std::string dir="private-control/";return dir; }
inline bool descriptor_logging()
{
		static std::atomic<u64> last_check{0};
		static std::atomic<bool> on{false};
		static std::atomic<u64> records{0};
		static std::mutex poll_mutex;
		const u64 now = get_system_time();
		if (now - last_check.load() > 500'000)
		{
			std::lock_guard lock(poll_mutex);
			if (now > last_check.load() && now - last_check.load() > 500'000)
			{
				on = fs::is_file(ctl_dir() + "d1descriptor");
				last_check = now;
			}
		}
		if (!on.load()) return false;
		constexpr u64 limit = 262'144;
		const u64 record = records.fetch_add(1);
		if (record == limit) d1_log.notice("D1DESC cap=%llu", limit);
		return record < limit;
	}
inline u64 next_descriptor_submit_order()
{
		static std::atomic<u64> order{0};
		return order.fetch_add(1) + 1;
	}
}
namespace vk {
class disposable_t
{
		void* ptr;
		std::function<void(void*)> deleter;

		disposable_t(void* ptr_, std::function<void(void*)> deleter_) :
			ptr(ptr_), deleter(deleter_) {}
	public:

		disposable_t() = delete;
		disposable_t(const disposable_t&) = delete;

		disposable_t(disposable_t&& other) noexcept :
			ptr(std::exchange(other.ptr, nullptr)),
			deleter(other.deleter)
		{}

		~disposable_t()
		{
			if (ptr)
			{
				deleter(ptr);
				ptr = nullptr;
			}
		}

		// Read-only identity for bounded lifetime diagnostics; no ownership transfer.
		const void* debug_handle() const { return ptr; }

		template <typename T>
		static disposable_t make(T* raw)
		{
			return disposable_t(raw, [](void* raw)
			{
				delete static_cast<T*>(raw);
			});
		}
	};

}

int checks=0,failures=0;
void check(const std::string& name,bool pass) {
    ++checks;if(!pass)++failures;
    std::cout<<(pass?"PASS ":"FAIL ")<<name<<"\n";
}
struct resource {
    int& destroyed;
    explicit resource(int& value):destroyed(value){}
    ~resource(){++destroyed;}
};
int main() {
    check("default trace is disabled",!d1diag::descriptor_logging());
    clock_us=600'000;
    check("missing marker keeps trace disabled",!d1diag::descriptor_logging());
    check("first eligible check polls the exact private marker once",file_checks==1);
    marker=true;clock_us=700'000;
    check("marker polling is throttled",!d1diag::descriptor_logging() && file_checks==1);
    clock_us=1'200'000;
    std::atomic<u64> accepted{d1diag::descriptor_logging()?1u:0u};
    check("next poll arms the trace",accepted==1 && file_checks==2);
    auto reserve=[&]{for(int i=0;i<140'000;++i) if(d1diag::descriptor_logging()) ++accepted;};
    std::thread worker_a(reserve),worker_b(reserve);worker_a.join();worker_b.join();
    check("concurrent callers share exactly one process budget",accepted==262'144);
    check("exhaustion is reported exactly once",cap_notices==1);
    check("concurrent records do not poll the filesystem again",file_checks==2);
    marker=false;clock_us=1'800'000;
    check("removing the marker disables logging",!d1diag::descriptor_logging());
    marker=true;clock_us=2'400'000;
    check("reactivation cannot reset the process budget",!d1diag::descriptor_logging() && cap_notices==1);

    std::vector<u64> orders_a,orders_b;
    auto orders=[](std::vector<u64>& values){for(int i=0;i<100'000;++i)values.push_back(d1diag::next_descriptor_submit_order());};
    std::thread a(orders,std::ref(orders_a)),b(orders,std::ref(orders_b));a.join();b.join();
    orders_a.insert(orders_a.end(),orders_b.begin(),orders_b.end());
    std::sort(orders_a.begin(),orders_a.end());
    bool unique=true;
    for(u64 i=0;i<orders_a.size();++i) unique &= orders_a[i]==i+1;
    check("concurrent ordinal reservations are unique and contiguous",unique);
    check("later submissions retain monotonic order",d1diag::next_descriptor_submit_order()==200'001);

    int destroyed=0;auto* object=new resource(destroyed);
    {
        auto first=vk::disposable_t::make(object);
        const auto& read_only=first;
        check("read-only diagnostic identity retains the original pointer",read_only.debug_handle()==object);
        check("identity inspection does not destroy the resource",destroyed==0);
        {
            auto second=std::move(first);
            check("move transfers identity without changing ownership",first.debug_handle()==nullptr && second.debug_handle()==object && destroyed==0);
        }
        check("moved resource is destroyed exactly once",destroyed==1);
    }
    check("moved-from disposal cannot destroy the resource twice",destroyed==1);
    std::cout<<checks<<" descriptor diagnostic checks, "<<failures<<" failed\n";
    return failures?1:0;
}
