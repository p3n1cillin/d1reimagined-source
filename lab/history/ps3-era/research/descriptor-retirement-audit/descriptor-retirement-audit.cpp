
#include <algorithm>
#include <atomic>
#include <cstdint>
#include <functional>
#include <iostream>
#include <limits>
#include <memory>
#include <mutex>
#include <new>
#include <vector>
using u32 = std::uint32_t; using u64 = std::uint64_t; using usz = std::size_t;
using VkDescriptorSet = u64;
template<class T> using atomic_t = std::atomic<T>;
constexpr u32 umax = std::numeric_limits<u32>::max();
namespace d1diag { unsigned g_tables_released = 0; }
namespace vk {
u64 current_eid = 0, completed_eid = 0;
u64 current_event_id() { return current_eid; }
u64 last_completed_event_id() { return completed_eid; }
std::vector<std::function<void()>> callbacks;
struct gc_callback_t { std::function<void()> m_callback; gc_callback_t(std::function<void()> f): m_callback(f) {} };
struct fake_gc {
    void dispose(std::unique_ptr<gc_callback_t>& object) {
        callbacks.push_back(std::move(object->m_callback)); object.reset();
    }
} collector;
fake_gc* get_gc() { return &collector; }
struct descriptor_pool {
    u32 m_current_subpool_index = 0;
atomic_t<u32> m_pending_resets = 0;
    unsigned resets = 0; bool pending_during_reset = false;
    void reset(u32, u32) { ++resets; pending_during_reset = has_pending_resets(); }
    void destroy() {}
    bool has_pending_resets() const { return m_pending_resets.load() != 0; }
    void next_subpool() { 
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
			vk::get_gc()->dispose(cleanup_obj);
		}

		 }
};
struct descriptor_set {
    u64 handle = 0;
    u64 value() const { return handle; }
    explicit operator bool() const { return handle != 0; }
    descriptor_set& operator=(u64 h) { handle = h; return *this; }
    void on_bind() {}
};
namespace glsl {
struct descriptor_table_t {
    u64 m_last_commit_frame = 0;
u64 m_last_commit_eid = 0;
    bool m_registered = false, m_any_descriptors_dirty = false;
    std::vector<bool> m_descriptors_dirty;
    std::vector<int> m_descriptor_template;
    u64 m_descriptor_template_cache_id = 0;
    std::unique_ptr<descriptor_pool> m_descriptor_pool;
    descriptor_set m_descriptor_set;
    bool allocation_flushes = false;
    u64 allocate_descriptor_set() {
        if (!m_descriptor_pool) m_descriptor_pool = std::make_unique<descriptor_pool>();
        if (allocation_flushes) ++current_eid;
        return current_eid + 100;
    }
    void create_descriptor_template() { m_descriptor_template.push_back(1); }
    void update_descriptor_template() {}
    u64 commit(); void release_resources();
};
std::mutex g_table_registry_mutex;
std::vector<descriptor_table_t*> g_table_registry;
atomic_t<u64> g_table_frame = 0;
void release_idle_descriptor_tables(){
			// Frame age is a retention policy, not proof that GPU use or reset callbacks finished.
			constexpr u64 idle_frames = 256;
			const u64 frame = ++g_table_frame;

			if (frame % 64)
			{
				return;
			}

			std::lock_guard lock(g_table_registry_mutex);
			usz kept = 0;

			for (auto* table : g_table_registry)
			{
				// Require a later completed submission: the zero watermark also means "none yet".
				// Reset callbacks can lag GPU completion, or have their GC scope delayed by async work.
				if (frame - table->m_last_commit_frame > idle_frames &&
					vk::last_completed_event_id() > table->m_last_commit_eid &&
					(!table->m_descriptor_pool || !table->m_descriptor_pool->has_pending_resets()))
				{
					table->release_resources();
					table->m_registered = false;
					d1diag::g_tables_released++;
					continue;
				}

				g_table_registry[kept++] = table;
			}

			g_table_registry.resize(kept);
		}
void descriptor_table_t::release_resources(){
			// Drop the current set (and its storage) and the pool; the next commit() allocates a new
			// set from a new pool and rewrites every descriptor from the template.
			m_descriptor_set.~descriptor_set();
			new (&m_descriptor_set) vk::descriptor_set();

			if (m_descriptor_pool)
			{
				m_descriptor_pool->destroy();
				m_descriptor_pool.reset();
			}

			m_descriptor_template_cache_id = umax;
		}
VkDescriptorSet descriptor_table_t::commit(){
			m_last_commit_frame = g_table_frame;

			if (!m_registered) [[unlikely]]
			{
				std::lock_guard lock(g_table_registry_mutex);
				g_table_registry.push_back(this);
				m_registered = true;
			}

			if (!m_descriptor_set)
			{
				m_any_descriptors_dirty = true;
				std::fill(m_descriptors_dirty.begin(), m_descriptors_dirty.end(), false);
			}

			// Check if we need to actually open a new set
			if (!m_any_descriptors_dirty)
			{
				m_last_commit_eid = vk::current_event_id();
				return m_descriptor_set.value();
			}

			m_descriptor_set = allocate_descriptor_set();

			if (!m_descriptor_template.empty()) [[ likely ]]
			{
				// Run pointer updates. Optimized for cached back-to-back updates which are quite frequent.
				update_descriptor_template();
			}
			else
			{
				// Creating the template also seeds initial values
				create_descriptor_template();
			}

			m_descriptor_set.on_bind();
			m_any_descriptors_dirty = false;
			// Allocation may flush/re-enter submission on fragmentation. Stamp its resulting scope.
			m_last_commit_eid = vk::current_event_id();

			return m_descriptor_set.value();
		}
}
}
int main() {
    using namespace vk; using namespace vk::glsl;
    int checks=0, failures=0;
    auto check=[&](const char* name, bool ok) { ++checks; if (!ok) { ++failures; std::cout << "FAIL " << name << '\n'; } };
    auto initialize=[&] { g_table_registry.clear(); callbacks.clear(); g_table_frame=0; current_eid=0; completed_eid=0; };
    auto sweep=[&](u64 frame) { g_table_frame=frame-1; release_idle_descriptor_tables(); };
    {
        initialize(); descriptor_table_t t; t.commit(); sweep(320);
        check("age alone cannot release an unsubmitted zero-event table", t.m_registered && t.m_descriptor_pool);
    }
    {
        initialize(); current_eid=10; descriptor_table_t t; t.commit();
        check("dirty commit tracks current submission scope", t.m_last_commit_eid==10);
        completed_eid=9; sweep(320);
        check("GPU behind last use retains idle pool", t.m_registered && t.m_descriptor_pool);
        completed_eid=10; sweep(384);
        check("equal watermark retains conservative next-submission margin", t.m_registered && t.m_descriptor_pool);
        completed_eid=11; sweep(448);
        check("later completion releases eligible idle pool", !t.m_registered && !t.m_descriptor_pool && !t.m_descriptor_set);
        check("release invalidates descriptor template storage cache", t.m_descriptor_template_cache_id==umax);
        current_eid=12; t.commit();
        check("released table reactivates with a pool and rewritten set", t.m_registered && t.m_descriptor_pool && t.m_descriptor_set);
        check("reactivation tracks the new event scope", t.m_last_commit_eid==12);
    }
    {
        initialize(); descriptor_table_t t; t.commit(); current_eid=25; t.commit();
        check("unchanged descriptor commit still tracks later use", t.m_last_commit_eid==25);
        completed_eid=20; sweep(320);
        check("unchanged set's later GPU use prevents release", t.m_registered && t.m_descriptor_pool);
    }
    {
        initialize(); current_eid=30; descriptor_table_t t; t.allocation_flushes=true; t.commit();
        check("allocation reentry records resulting event scope", t.m_last_commit_eid==31);
        completed_eid=31; sweep(320);
        check("allocation reentry cannot retire at its own watermark", t.m_registered && t.m_descriptor_pool);
    }
    {
        initialize(); descriptor_pool pool; pool.next_subpool(); pool.next_subpool();
        check("all queued pool resets retain their owner", pool.has_pending_resets());
        auto first=std::move(callbacks[0]); auto second=std::move(callbacks[1]); callbacks.clear();
        first(); check("one remaining reset keeps owner retained", pool.has_pending_resets() && pool.resets==1);
        check("owner remains retained while reset executes", pool.pending_during_reset);
        second(); check("only finished final reset clears retention", !pool.has_pending_resets() && pool.resets==2);
        pool.m_current_subpool_index=umax; pool.next_subpool();
        check("absent old subpool queues no reset", callbacks.empty() && !pool.has_pending_resets());
    }
    {
        initialize(); current_eid=40; descriptor_table_t t; t.commit(); t.m_descriptor_pool->next_subpool();
        completed_eid=100; sweep(320);
        check("GPU completion with delayed GC retains idle callback owner", t.m_registered && t.m_descriptor_pool);
        if (t.m_descriptor_pool) { auto callback=std::move(callbacks[0]); callbacks.clear(); callback(); }
        else callbacks.clear(); // Never execute a dangling baseline callback in a CPU fixture.
        sweep(384); check("finished delayed callback allows idle release", !t.m_registered && !t.m_descriptor_pool);
    }
    {
        initialize(); current_eid=2; completed_eid=20; descriptor_table_t t; t.commit();
        sweep(256); check("idle threshold remains strict", t.m_registered && t.m_descriptor_pool);
        sweep(319); check("non-sweep frame retains eligible pool", t.m_registered && t.m_descriptor_pool);
        sweep(320); check("eligible pool is eventually reclaimed", !t.m_registered && !t.m_descriptor_pool);
    }
    {
        initialize(); current_eid=5; descriptor_table_t old, recent; old.commit(); recent.commit();
        g_table_frame=300; current_eid=10; recent.commit(); completed_eid=20; sweep(320);
        check("registry compaction retains a recently used table", !old.m_registered && recent.m_registered && g_table_registry.size()==1 && g_table_registry[0]==&recent);
    }
    std::cout << checks << " descriptor retirement checks, " << failures << " failed\n";
    return failures ? 1 : 0;
}
