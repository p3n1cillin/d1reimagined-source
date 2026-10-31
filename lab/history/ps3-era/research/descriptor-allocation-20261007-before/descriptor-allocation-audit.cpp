
#include <algorithm>
#include <atomic>
#include <bit>
#include <cstdint>
#include <functional>
#include <iostream>
#include <limits>
#include <map>
#include <memory>
#include <mutex>
#include <stdexcept>
#include <utility>
#include <vector>
using u32=std::uint32_t; using u64=std::uint64_t;
using VkDescriptorPool=u64; using VkDescriptorSet=u64; using VkDescriptorSetLayout=u64;
using VkBool32=u32; using VkDescriptorPoolResetFlags=u32; using VkResult=int;
constexpr u32 umax=std::numeric_limits<u32>::max();
constexpr u64 VK_NULL_HANDLE=0;
constexpr u32 VK_TRUE=1, VK_FALSE=0, VK_DESCRIPTOR_POOL_CREATE_UPDATE_AFTER_BIND_BIT=1;
constexpr u32 VK_STRUCTURE_TYPE_DESCRIPTOR_POOL_CREATE_INFO=1, VK_STRUCTURE_TYPE_DESCRIPTOR_SET_ALLOCATE_INFO=2;
constexpr int VK_SUCCESS=0, VK_ERROR_FRAGMENTATION_EXT=-1;
#define FORCE_INLINE inline
template<class T> using atomic_t=std::atomic<T>;
using shared_mutex=std::mutex; using reader_lock=std::lock_guard<std::mutex>;
void ensure(bool ok) { if (!ok) throw std::runtime_error("assertion"); }
#define CHECK_RESULT(r) ensure((r)==VK_SUCCESS)
struct logger { template<class... T> void notice(const char*,T...) {} template<class... T> void warning(const char*,T...) {} } d1_log,rsx_log;
namespace d1diag { bool descriptor_logging() { return false; } }
namespace fmt { void throw_exception(const char*) { throw std::runtime_error("unreachable"); } }
struct VkDescriptorPoolSize { u32 type=0,descriptorCount=0; };
struct VkDescriptorPoolCreateInfo { u32 sType=0; const void* pNext=nullptr; u32 flags=0,maxSets=0,poolSizeCount=0; const VkDescriptorPoolSize* pPoolSizes=nullptr; };
struct VkDescriptorSetAllocateInfo { u32 sType=0; const void* pNext=nullptr; VkDescriptorPool descriptorPool=0; u32 descriptorSetCount=0; const VkDescriptorSetLayout* pSetLayouts=nullptr; };
namespace rsx {
template<class T> struct simple_array: std::vector<T> {
    using std::vector<T>::vector;
    T pop_back() { T item=this->back(); std::vector<T>::pop_back(); return item; }
    template<class F> auto map(F f) const {
        simple_array<decltype(f(std::declval<T>()))> result;
        for (const auto& item:*this) result.push_back(f(item));
        return result;
    }
};
}
struct pool_state { u32 size=0,used=0,generation=0; bool destroyed=false; };
struct set_state { VkDescriptorPool pool=0; VkDescriptorSetLayout layout=0; u32 generation=0; };
std::map<VkDescriptorPool,pool_state> pools;
std::map<VkDescriptorSet,set_state> sets;
u64 next_pool=1,next_set=10000; unsigned allocation_calls=0;
VkResult vkCreateDescriptorPool(u64,const VkDescriptorPoolCreateInfo* info,const void*,VkDescriptorPool* pool) {
    *pool=next_pool++; pools[*pool]={info->maxSets}; return VK_SUCCESS;
}
void vkDestroyDescriptorPool(u64,VkDescriptorPool pool,const void*) { pools.at(pool).destroyed=true; }
VkResult vkResetDescriptorPool(u64,VkDescriptorPool pool,u32) { auto& p=pools.at(pool); p.used=0; ++p.generation; return VK_SUCCESS; }
VkResult vkAllocateDescriptorSets(u64,const VkDescriptorSetAllocateInfo* info,VkDescriptorSet* result) {
    auto& p=pools.at(info->descriptorPool); ensure(!p.destroyed && p.used+info->descriptorSetCount<=p.size);
    ++allocation_calls; p.used+=info->descriptorSetCount;
    for (u32 i=0;i<info->descriptorSetCount;++i) { result[i]=next_set++; sets[result[i]]={info->descriptorPool,info->pSetLayouts[i],p.generation}; }
    return VK_SUCCESS;
}
bool valid(VkDescriptorSet set) { const auto& s=sets.at(set); const auto& p=pools.at(s.pool); return !p.destroyed && s.generation==p.generation; }
namespace vk {
struct render_device { operator u64() const { return 1; } u64 get_descriptor_update_after_bind_support() const { return 0; } } device;
u64 current_event_id() { return 1; } u64 last_completed_event_id() { return 0; }
void on_descriptor_pool_fragmentation(bool) {} void die_with_error(int) { throw std::runtime_error("Vulkan error"); }
struct gc_callback_t { std::function<void()> callback; explicit gc_callback_t(std::function<void()> f):callback(std::move(f)) {} ~gc_callback_t() { callback(); } };
struct collector {
    std::vector<std::unique_ptr<gc_callback_t>> pending;
    void dispose(std::unique_ptr<gc_callback_t>& object) { pending.push_back(std::move(object)); }
    void complete() { pending.clear(); }
} gc;
collector* get_gc() { return &gc; }
class descriptor_pool {
	public:
		descriptor_pool() = default;
		~descriptor_pool() = default;

		void create(const vk::render_device& dev, const rsx::simple_array<VkDescriptorPoolSize>& pool_sizes, u32 min_sets = 1024, u32 max_sets = 1024);
		void destroy();

		VkDescriptorSet allocate(VkDescriptorSetLayout layout, VkBool32 use_cache = VK_TRUE);

		operator VkDescriptorPool() { return m_current_pool_handle; }
		FORCE_INLINE bool valid() const { return !m_device_subpools.empty(); }
		FORCE_INLINE u32 max_sets() const { return m_current_subpool_index >= m_device_subpools.size() ? 0u : m_device_subpools[m_current_subpool_index].size; }
		FORCE_INLINE bool has_pending_resets() const { return m_pending_resets.load() != 0; }

	public:
		FORCE_INLINE bool can_allocate(u32 required_count, u32 already_used_count = 0) const { return (required_count + already_used_count) <= max_sets(); };
		void reset(u32 subpool_id, VkDescriptorPoolResetFlags flags);
		void next_subpool();

		std::pair<VkResult, VkDescriptorPool> new_subpool();

		struct logical_subpool_t
		{
			VkDescriptorPool handle = VK_NULL_HANDLE;
			u32 size = 0;
			VkBool32 busy = VK_FALSE;
		};

		struct autoscaling_config_t
		{
			u32 min_pool_size = 0;
			u32 max_pool_size = 0;
			u32 current_size = 0;

			// Debounce setup.
			static constexpr u32 increment_min_steps = 2u;
			u32 increment_steps = 0;

			u32 get_pool_size();
		};

		const vk::render_device* m_owner = nullptr;
		VkDescriptorPoolCreateInfo m_create_info = {};
		autoscaling_config_t m_autoscaling_config = {};
		rsx::simple_array<VkDescriptorPoolSize> m_create_info_pool_sizes;

		rsx::simple_array<logical_subpool_t> m_device_subpools;
		VkDescriptorPool m_current_pool_handle = VK_NULL_HANDLE;
		u32 m_current_subpool_index = umax;
		u32 m_current_subpool_offset = 0;

		shared_mutex m_subpool_lock;
		// Deferred reset callbacks retain this pool until reset() has returned.
		atomic_t<u32> m_pending_resets = 0;

		static constexpr size_t max_cache_size = 64;
		VkDescriptorSetLayout m_cached_layout = VK_NULL_HANDLE;
		rsx::simple_array<VkDescriptorSet> m_descriptor_set_cache;
		rsx::simple_array<VkDescriptorSetLayout> m_allocation_request_cache;
	};
u32 descriptor_pool::autoscaling_config_t::get_pool_size()
	{
		if (current_size < min_pool_size)
		{
			current_size = min_pool_size;
			return min_pool_size;
		}

		if (current_size >= max_pool_size)
		{
			current_size = max_pool_size;
			return max_pool_size;
		}

		// Try grow
		if ((increment_steps++) < (increment_min_steps - 1u))
		{
			return current_size;
		}

		increment_steps = 0u;
		current_size = std::min(current_size * 2u, max_pool_size);
		return current_size;
	}
void descriptor_pool::create(const vk::render_device& dev, const rsx::simple_array<VkDescriptorPoolSize>& pool_sizes, u32 min_sets, u32 max_sets)
	{
		m_autoscaling_config.min_pool_size = std::max(min_sets, 2u);
		m_autoscaling_config.max_pool_size = std::max(min_sets, max_sets);

		ensure(m_autoscaling_config.max_pool_size >= 16u);

		m_create_info_pool_sizes = pool_sizes;

		m_create_info.flags = dev.get_descriptor_update_after_bind_support() ? VK_DESCRIPTOR_POOL_CREATE_UPDATE_AFTER_BIND_BIT : 0;
		m_create_info.maxSets = max_sets;
		m_create_info.poolSizeCount = m_create_info_pool_sizes.size();
		m_create_info.pPoolSizes = m_create_info_pool_sizes.data();
		m_create_info.sType = VK_STRUCTURE_TYPE_DESCRIPTOR_POOL_CREATE_INFO;

		m_owner = &dev;
		next_subpool();
	}
void descriptor_pool::destroy()
	{
		if (m_device_subpools.empty()) return;

		for (auto& pool : m_device_subpools)
		{
			vkDestroyDescriptorPool((*m_owner), pool.handle, nullptr);
			pool.handle = VK_NULL_HANDLE;
		}

		m_owner = nullptr;
	}
void descriptor_pool::reset(u32 subpool_id, VkDescriptorPoolResetFlags flags)
	{
		std::lock_guard lock(m_subpool_lock);

		if (d1diag::descriptor_logging())
		{
			d1_log.notice("D1DESC reset pool=%016llx index=%u current=%llu completed=%llu",
				std::bit_cast<u64>(m_device_subpools[subpool_id].handle), subpool_id, current_event_id(), last_completed_event_id());
		}
		CHECK_RESULT(vkResetDescriptorPool(*m_owner, m_device_subpools[subpool_id].handle, flags));
		m_device_subpools[subpool_id].busy = VK_FALSE;
	}
VkDescriptorSet descriptor_pool::allocate(VkDescriptorSetLayout layout, VkBool32 use_cache)
	{
		if (use_cache)
		{
			if (m_descriptor_set_cache.empty())
			{
				// For optimal cache utilization, each pool should only allocate one layout
				m_cached_layout = layout;
			}
			else if (m_cached_layout != layout)
			{
				use_cache = VK_FALSE;
			}
			else
			{
				return m_descriptor_set_cache.pop_back();
			}
		}

		if (!can_allocate(use_cache ? 4 : 1, m_current_subpool_offset))
		{
			next_subpool();
		}

		VkDescriptorSet new_descriptor_set;
		VkDescriptorSetAllocateInfo alloc_info = {};
		alloc_info.sType = VK_STRUCTURE_TYPE_DESCRIPTOR_SET_ALLOCATE_INFO;
		alloc_info.descriptorPool = m_current_pool_handle;
		alloc_info.descriptorSetCount = 1;
		alloc_info.pSetLayouts = &layout;

		if (use_cache)
		{
			const auto alloc_size = std::min<u32>(max_sets() - m_current_subpool_offset, max_cache_size);
			m_allocation_request_cache.resize(alloc_size);
			for (auto& layout_ : m_allocation_request_cache)
			{
				layout_ = m_cached_layout;
			}

			ensure(m_descriptor_set_cache.empty());
			alloc_info.descriptorSetCount = alloc_size;
			alloc_info.pSetLayouts = m_allocation_request_cache.data();

			m_descriptor_set_cache.resize(alloc_size);
			CHECK_RESULT(vkAllocateDescriptorSets(*m_owner, &alloc_info, m_descriptor_set_cache.data()));

			m_current_subpool_offset += alloc_size;
			new_descriptor_set = m_descriptor_set_cache.pop_back();
		}
		else
		{
			m_current_subpool_offset++;
			CHECK_RESULT(vkAllocateDescriptorSets(*m_owner, &alloc_info, &new_descriptor_set));
		}

		return new_descriptor_set;
	}
void descriptor_pool::next_subpool()
	{
		if (m_current_subpool_index != umax)
		{
			// Enqueue release using gc
			m_pending_resets++;
			auto release_func = [subpool_index=m_current_subpool_index, this]()
			{
				this->reset(subpool_index, 0);
				m_pending_resets--;
			};

			auto cleanup_obj = std::make_unique<gc_callback_t>(release_func);
			if (d1diag::descriptor_logging())
			{
				d1_log.notice("D1DESC retire object=%016llx pool=%016llx index=%u current=%llu completed=%llu",
					std::bit_cast<u64>(cleanup_obj.get()), std::bit_cast<u64>(m_current_pool_handle),
					m_current_subpool_index, current_event_id(), last_completed_event_id());
			}
			vk::get_gc()->dispose(cleanup_obj);
		}

		m_current_subpool_offset = 0;
		m_current_subpool_index = umax;

		const int max_retries = 2;
		int retries = max_retries;

		do
		{
			// Happy path - maybe we have a free subpool
			{
				reader_lock lock(m_subpool_lock);

				for (u32 index = 0; index < m_device_subpools.size(); ++index)
				{
					if (!m_device_subpools[index].busy)
					{
						m_current_subpool_index = index;
						m_device_subpools[m_current_subpool_index].busy = VK_TRUE;
						m_current_pool_handle = m_device_subpools[m_current_subpool_index].handle;
						break;
					}
				}

				if (m_current_subpool_index != umax)
				{
					return;
				}
			}

			const auto [result, subpool] = new_subpool();
			if (result != VK_SUCCESS)
			{
				if (retries-- && (result == VK_ERROR_FRAGMENTATION_EXT))
				{
					rsx_log.warning("Descriptor pool creation failed with fragmentation error. Will attempt to recover.");
					vk::on_descriptor_pool_fragmentation(!retries);
					continue;
				}

				vk::die_with_error(result);
				fmt::throw_exception("Unreachable");
			}

			// New subpool created successfully
			std::lock_guard lock(m_subpool_lock);

			m_device_subpools.push_back(
			{
				.handle = subpool,
				.size = m_autoscaling_config.current_size,
				.busy = VK_TRUE
			});

			m_current_subpool_index = m_device_subpools.size() - 1;
			m_current_pool_handle = m_device_subpools[m_current_subpool_index].handle;

		} while (m_current_subpool_index == umax);
	}
std::pair<VkResult, VkDescriptorPool> descriptor_pool::new_subpool()
	{
		// Try autoscaling
		const auto prev_scaling_config = m_autoscaling_config;
		const u32 set_count = m_autoscaling_config.get_pool_size();

		// Configure request using current pool size
		auto descriptor_pool_sizes = m_create_info_pool_sizes.map([set_count](const VkDescriptorPoolSize& pool_size_info)
		{
			auto ret = pool_size_info;
			ret.descriptorCount *= set_count;
			return ret;
		});

		m_create_info.maxSets = set_count;
		m_create_info.poolSizeCount = descriptor_pool_sizes.size();
		m_create_info.pPoolSizes = descriptor_pool_sizes.data();

		VkDescriptorPool subpool = VK_NULL_HANDLE;
		VkResult result = vkCreateDescriptorPool(*m_owner, &m_create_info, nullptr, &subpool);

		if (result != VK_SUCCESS)
		{
			// Roll back autoscaling
			m_autoscaling_config = prev_scaling_config;
		}

		// Cleanup
		m_create_info.pPoolSizes = nullptr;
		m_create_info.poolSizeCount = 0;

		return { result, subpool };
	}
}
int main() {
    int checks=0,failures=0;
    auto check=[&](const char* name,bool ok) { ++checks; if(!ok) { ++failures; std::cout<<"FAIL "<<name<<'\n'; } };
    auto finish=[](vk::descriptor_pool& p) { vk::gc.complete(); p.destroy(); };
    const rsx::simple_array<VkDescriptorPoolSize> sizes{{0,1}};
    {
        vk::descriptor_pool p; p.create(vk::device,sizes,128,128);
        const auto first=p.allocate(10), pool=sets.at(first).pool;
        const auto calls=allocation_calls;
        const auto second=p.allocate(10);
        check("same-layout cache avoids another Vulkan allocation",allocation_calls==calls && first!=second);
        check("cached set retains current pool and layout",sets.at(second).pool==pool && sets.at(second).layout==10 && valid(second));
        const auto other=p.allocate(20);
        check("other layout gets its requested layout",sets.at(other).layout==20 && valid(other));
        const auto cached=p.allocate(10);
        check("mixed layout inside one pool preserves valid cached entries",sets.at(cached).pool==pool && sets.at(cached).layout==10 && valid(cached));
        finish(p);
    }
    {
        vk::descriptor_pool p; p.create(vk::device,sizes,128,128);
        const auto first=p.allocate(10), old=sets.at(first).pool;
        for(unsigned i=0;i<65;++i) p.allocate(20);
        check("mixed-layout exhaustion switches pools",p.m_current_pool_handle!=old && p.has_pending_resets());
        const auto fresh=p.allocate(10);
        check("cached handles cannot escape a retired pool",sets.at(fresh).pool==p.m_current_pool_handle);
        check("post-switch cached allocation has requested layout",sets.at(fresh).layout==10);
        vk::gc.complete();
        check("old pool reset cannot invalidate post-switch allocation",valid(fresh));
        check("retired source allocation is invalidated after reset",!valid(first));
        const auto reused=p.allocate(10);
        check("new pool cache remains valid after old asynchronous reset",valid(reused) && sets.at(reused).pool==p.m_current_pool_handle);
        finish(p);
    }
    {
        vk::descriptor_pool p; p.create(vk::device,sizes,128,128);
        const auto first=p.allocate(10), old=sets.at(first).pool;
        for(unsigned i=0;i<65;++i) p.allocate(10,VK_FALSE);
        check("uncached exhaustion switches pools",p.m_current_pool_handle!=old);
        const auto fresh=p.allocate(10);
        check("uncached spill also drops old cached handles",sets.at(fresh).pool==p.m_current_pool_handle);
        vk::gc.complete();
        check("uncached spill allocation survives old reset",valid(fresh));
        finish(p);
    }
    {
        vk::descriptor_pool p; p.create(vk::device,sizes,128,128);
        const auto first=p.allocate(10), old=sets.at(first).pool;
        for(unsigned i=0;i<128;++i) p.allocate(10);
        const auto fresh=p.allocate(10);
        check("single-layout rollover allocates valid current-pool sets",p.m_current_pool_handle!=old && valid(fresh) && sets.at(fresh).pool==p.m_current_pool_handle);
        vk::gc.complete();
        check("single-layout rollover survives old reset",valid(fresh));
        finish(p);
    }
    {
        vk::descriptor_pool p; p.create(vk::device,sizes,128,128);
        p.allocate(10); const auto old=p.m_current_pool_handle;
        for(unsigned i=0;i<65;++i) p.allocate(20);
        vk::gc.complete();
        for(unsigned i=0;i<128;++i) p.allocate(20,VK_FALSE);
        const auto fresh=p.allocate(30);
        check("reclaimed pools remain usable after rollover",valid(fresh) && sets.at(fresh).pool==p.m_current_pool_handle);
        check("reclaimed pool is selected with reset capacity",p.m_current_pool_handle==old && pools.at(old).generation==1);
        check("new cache after reuse uses requested layout",sets.at(fresh).layout==30);
        finish(p);
    }
    {
        vk::descriptor_pool p; p.create(vk::device,sizes,2,64);
        bool correct=true;
        for(unsigned i=0;i<150;++i) { const auto set=p.allocate(10); correct &= valid(set) && sets.at(set).layout==10 && sets.at(set).pool==p.m_current_pool_handle; vk::gc.complete(); }
        check("autoscaling preserves allocation provenance",correct);
        check("autoscaling grows within configured bounds",p.max_sets()>=4 && p.max_sets()<=64);
        finish(p);
    }
    std::cout<<checks<<" descriptor allocation checks, "<<failures<<" failed\n";
    return failures ? 1 : 0;
}
