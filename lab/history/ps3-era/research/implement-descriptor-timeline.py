from pathlib import Path
import shutil
root=Path(r'F:\d1\<pc>-import\dev\rpcs3')
base=Path(r'F:\d1\<pc>-import\d1xenia\local\RESEARCH\gpu-descriptor-timeline-baseline')
files=['rpcs3/Emu/RSX/d1diag.h','rpcs3/Emu/RSX/VK/VKCommandStream.cpp','rpcs3/Emu/RSX/VK/VKGSRenderTypes.hpp','rpcs3/Emu/RSX/VK/VKResourceManager.h','rpcs3/Emu/RSX/VK/vkutils/descriptors.cpp','rpcs3/Emu/RSX/VK/vkutils/garbage_collector.h']
base.mkdir(exist_ok=False)
for relative in files:
    dest=base/relative; dest.parent.mkdir(parents=True,exist_ok=True); shutil.copy2(root/relative,dest)
def edit(relative,old,new):
    p=root/relative; s=p.read_text(encoding='utf-8'); assert s.count(old)==1,(relative,s.count(old),old); p.write_text(s.replace(old,new),encoding='utf-8',newline='\n')
edit(files[0],'#include <atomic>','#include <atomic>\n#include <bit>')
edit(files[0],'\t// Timeline tracing, active while',r'''	// Opt-in descriptor lifetime trace. One process-wide budget, including toggles;
	// each successful call reserves one record. Runtime handles are private evidence.
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

	// Called inside the queue-submit guard to identify physical submission order.
	inline u64 next_descriptor_submit_order()
	{
		static std::atomic<u64> order{0};
		return order.fetch_add(1) + 1;
	}

	// Timeline tracing, active while''')
edit(files[1],'\t\t\tresult = vkQueueSubmit(submit_info.queue, 1, &info, submit_info.pfence->handle);',r'''			result = vkQueueSubmit(submit_info.queue, 1, &info, submit_info.pfence->handle);
			if (d1diag::descriptor_logging())
			{
				d1_log.notice("D1DESC submit order=%llu queue=%016llx cmd=%016llx fence=%016llx result=%d",
					d1diag::next_descriptor_submit_order(), std::bit_cast<u64>(submit_info.queue),
					std::bit_cast<u64>(submit_info.commands), std::bit_cast<u64>(submit_info.pfence->handle), static_cast<int>(result));
			}''')
edit(files[2],'\t\t\teid_tag = vk::get_event_id();',r'''			eid_tag = vk::get_event_id();
			if (d1diag::descriptor_logging())
			{
				d1_log.notice("D1DESC tag eid=%llu cmd=%016llx fence=%016llx", eid_tag,
					std::bit_cast<u64>(static_cast<VkCommandBuffer>(*this)), std::bit_cast<u64>(m_submit_fence->handle));
			}''')
edit(files[2],'\t\t\t\t\tm_submit_fence->reset();\n\t\t\t\t\tvk::on_event_completed(eid_tag);',r'''					if (d1diag::descriptor_logging())
					{
						d1_log.notice("D1DESC completed path=poke eid=%llu cmd=%016llx fence=%016llx", eid_tag,
							std::bit_cast<u64>(static_cast<VkCommandBuffer>(*this)), std::bit_cast<u64>(m_submit_fence->handle));
					}
					m_submit_fence->reset();
					vk::on_event_completed(eid_tag);''')
edit(files[2],'\t\t\t\tm_submit_fence->reset();\n\t\t\t\tvk::on_event_completed(eid_tag);',r'''				if (d1diag::descriptor_logging())
				{
					d1_log.notice("D1DESC completed path=wait eid=%llu cmd=%016llx fence=%016llx", eid_tag,
						std::bit_cast<u64>(static_cast<VkCommandBuffer>(*this)), std::bit_cast<u64>(m_submit_fence->handle));
				}
				m_submit_fence->reset();
				vk::on_event_completed(eid_tag);''')
edit(files[3],'#include "Utilities/mutex.h"','#include "Utilities/mutex.h"\n#include "Emu/RSX/d1diag.h"')
edit(files[3],'\t\t\tget_current_eid_scope().m_disposables.emplace_back(std::move(disposable));',r'''			auto& scope = get_current_eid_scope();
			if (d1diag::descriptor_logging())
			{
				d1_log.notice("D1DESC dispose object=%016llx scope=%llu current=%llu completed=%llu",
					std::bit_cast<u64>(disposable.debug_handle()), scope.eid, current_event_id(), last_completed_event_id());
			}
			scope.m_disposables.emplace_back(std::move(disposable));''')
edit(files[3],'\t\t\t\t// Take all of the entries in one sweep',r'''				if (d1diag::descriptor_logging())
				{
					d1_log.notice("D1DESC collect cutoff=%llu first=%llu last=%llu", eid,
						m_eid_map.front().eid, std::prev(newest_it)->eid);
				}
				// Take all of the entries in one sweep''')
edit(files[4],'#include "garbage_collector.h"','#include "garbage_collector.h"\n#include "../VKResourceManager.h"')
edit(files[4],'\t\tCHECK_RESULT(vkResetDescriptorPool(*m_owner, m_device_subpools[subpool_id].handle, flags));',r'''		if (d1diag::descriptor_logging())
		{
			d1_log.notice("D1DESC reset pool=%016llx index=%u current=%llu completed=%llu",
				std::bit_cast<u64>(m_device_subpools[subpool_id].handle), subpool_id, current_event_id(), last_completed_event_id());
		}
		CHECK_RESULT(vkResetDescriptorPool(*m_owner, m_device_subpools[subpool_id].handle, flags));''')
edit(files[4],'\t\t\tauto cleanup_obj = std::make_unique<gc_callback_t>(release_func);',r'''			auto cleanup_obj = std::make_unique<gc_callback_t>(release_func);
			if (d1diag::descriptor_logging())
			{
				d1_log.notice("D1DESC retire object=%016llx pool=%016llx index=%u current=%llu completed=%llu",
					std::bit_cast<u64>(cleanup_obj.get()), std::bit_cast<u64>(m_current_pool_handle),
					m_current_subpool_index, current_event_id(), last_completed_event_id());
			}''')
edit(files[5],'\t\ttemplate <typename T>\n\t\tstatic disposable_t make(T* raw)',r'''		// Read-only identity for bounded lifetime diagnostics; no ownership transfer.
		const void* debug_handle() const { return ptr; }

		template <typename T>
		static disposable_t make(T* raw)''')
print('Saved exact six-file baseline and installed opt-in descriptor diagnostics')