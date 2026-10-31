
#include <cstdint>
#include <deque>
#include <functional>
#include <iostream>
#include <memory>
#include <stdexcept>
#include <string>
#include <utility>
#include <vector>
using u32=std::uint32_t; using u64=std::uint64_t; using VkResult=int;
constexpr VkResult VK_SUCCESS=0, VK_NOT_READY=1, VK_TIMEOUT=2;
constexpr VkResult VK_ERROR_OUT_OF_HOST_MEMORY=-1, VK_ERROR_OUT_OF_DEVICE_MEMORY=-2;
constexpr VkResult VK_ERROR_DEVICE_LOST=-4, VK_ERROR_UNKNOWN=-13;
constexpr VkResult VK_ERROR_VALIDATION_FAILED=-1000011001;
constexpr int VK_TRUE=1;
constexpr u64 FRAME_PRESENT_TIMEOUT=10000000ull, GENERAL_WAIT_TIMEOUT=2000000ull;
struct api_error { VkResult result; };
#define CHECK_RESULT(expr) do { const VkResult result_=(expr); if(result_!=VK_SUCCESS) throw api_error{result_}; } while(false)
#define ensure(expr) do { if(!(expr)) throw std::runtime_error("ensure"); } while(false)
struct logger { template<class... T> void warning(const char*,T...){}; } rsx_log;
struct shared_mutex { int readers=0; bool writer=false; std::function<void()> on_upgrade; };
struct reader_lock {
    shared_mutex& guard; bool exclusive=false;
    explicit reader_lock(shared_mutex& value):guard(value){++guard.readers;}
    void upgrade(){--guard.readers; if(guard.on_upgrade)guard.on_upgrade(); guard.writer=true;exclusive=true;}
    ~reader_lock(){if(exclusive)guard.writer=false;else --guard.readers;}
};
std::deque<VkResult> results;
std::vector<u64> wait_timeouts, completed;
int fence_resets=0, buffer_resets=0, buffer_begins=0, unsafe_resets=0;
int unsafe_fence_destruction=0, trims=0, globals_reset=0, heap_restores=0, frame_advances=0;
VkResult poke_status=VK_NOT_READY;
namespace vk {
struct fence {
    bool flushed=true, gpu_pending=false, track_destruction=false; int handle=1;
    fence()=default;
    explicit fence(int):track_destruction(true){}
    ~fence(){if(track_destruction && gpu_pending)++unsafe_fence_destruction;}
    void reset(){++fence_resets;if(gpu_pending)++unsafe_resets;flushed=false;}
    void wait_flush(){}
};
struct command_pool { int get_owner()const{return 1;} };
struct queue_submit_t { int queue; fence* pfence; };
struct command_buffer {
    bool is_pending=false;
    fence own_fence; fence* m_submit_fence=&own_fence; fence* external_fence=nullptr;
    command_pool own_pool; command_pool* pool=&own_pool;
    void reset(){++buffer_resets;if(is_pending || (external_fence && external_fence->gpu_pending))++unsafe_resets;}
    void begin(){++buffer_begins;}
    void submit(queue_submit_t& info,int){external_fence=info.pfence;info.pfence->gpu_pending=true;}
};
struct gc_fixture {
    std::vector<std::unique_ptr<fence>> held;
    void dispose(std::unique_ptr<fence>& value){held.push_back(std::move(value));}
    void clear(){for(auto& value:held)value->gpu_pending=false;held.clear();}
} gc;
gc_fixture* get_gc(){return &gc;}
VkResult wait_for_fence(fence* value,u64 timeout=0){
    wait_timeouts.push_back(timeout);
    VkResult result=results.empty()?VK_SUCCESS:results.front();
    if(!results.empty())results.pop_front();
    if(result==VK_SUCCESS)value->gpu_pending=false;
    // The real helper reports errors itself on the infinite polling path.
    if(!timeout && result!=VK_SUCCESS)throw api_error{result};
    return result;
}
u64 get_event_id(){return 42;}
void on_event_completed(u64 value){completed.push_back(value);}
}
VkResult vkGetFenceStatus(int,int){return poke_status;}
namespace vk {
struct command_buffer_chunk : public vk::command_buffer
{
		u64 eid_tag = 0;
		u64 reset_id = 0;
		shared_mutex guard_mutex;

		command_buffer_chunk() = default;

		inline void tag()
		{
			eid_tag = vk::get_event_id();
		}

		void reset()
		{
			if (is_pending && !poke())
			{
				wait(FRAME_PRESENT_TIMEOUT);
			}

			++reset_id;
			vk::command_buffer::reset();
		}

		bool poke()
		{
			reader_lock lock(guard_mutex);

			if (!is_pending)
			{
				return true;
			}

			if (!m_submit_fence->flushed)
			{
				return false;
			}

			if (vkGetFenceStatus(pool->get_owner(), m_submit_fence->handle) == VK_SUCCESS)
			{
				lock.upgrade();

				if (is_pending)
				{
					m_submit_fence->reset();
					vk::on_event_completed(eid_tag);

					is_pending = false;
					eid_tag = 0;
				}
			}

			return !is_pending;
		}

		VkResult wait(u64 timeout = 0ull)
		{
			reader_lock lock(guard_mutex);

			if (!is_pending)
			{
				return VK_SUCCESS;
			}

			const auto ret = vk::wait_for_fence(m_submit_fence, timeout);

			lock.upgrade();

			if (is_pending)
			{
				m_submit_fence->reset();
				vk::on_event_completed(eid_tag);

				is_pending = false;
				eid_tag = 0;
			}

			return ret;
		}

		void flush()
		{
			reader_lock lock(guard_mutex);

			if (!is_pending)
			{
				return;
			}

			m_submit_fence->wait_flush();
		}
	};

struct frame_context_t { command_buffer_chunk* swap_command_buffer=nullptr; u64 last_frame_sync_time=10; int heap_snapshot=1; };
struct resource_manager { void trim(){++trims;} } resources;
resource_manager* get_resource_manager(){return &resources;}
void reset_global_resources(){++globals_reset;}
void advance_completed_frame_counter(){++frame_advances;}
struct data_heap_manager { static void restore_snapshot(int){++heap_restores;} };
struct ui_overlay_renderer { void remove_temp_resources(u32){} };
template<class T> T* get_overlay_pass(){static T value;return &value;}
}
struct overlay_view { u32 uid=1; };
struct overlay_manager {
    std::vector<overlay_view*> dirty;
    bool has_dirty(){return !dirty.empty();}
    void lock_shared(){} void unlock_shared(){}
    auto& get_dirty(){return dirty;}
    void dispose(const std::vector<u32>&){}
};
struct VKGSRender {
    bool swapchain_unavailable=false;
    overlay_manager* m_overlay_manager=nullptr;
    u64 m_last_heap_sync_time=0;
    std::deque<vk::frame_context_t*> m_queued_frames;
    void frame_context_cleanup(vk::frame_context_t *ctx);
};
void VKGSRender::frame_context_cleanup(vk::frame_context_t *ctx)
{
	ensure(ctx->swap_command_buffer);

	// Perform hard swap here
	if (ctx->swap_command_buffer->wait(FRAME_PRESENT_TIMEOUT) != VK_SUCCESS)
	{
		// Lost surface/device, release swapchain
		swapchain_unavailable = true;
	}

	// Resource cleanup.
	{
		if (m_overlay_manager && m_overlay_manager->has_dirty())
		{
			auto ui_renderer = vk::get_overlay_pass<vk::ui_overlay_renderer>();
			m_overlay_manager->lock_shared();

			std::vector<u32> uids_to_dispose;
			uids_to_dispose.reserve(m_overlay_manager->get_dirty().size());

			for (const auto& view : m_overlay_manager->get_dirty())
			{
				ui_renderer->remove_temp_resources(view->uid);
				uids_to_dispose.push_back(view->uid);
			}

			m_overlay_manager->unlock_shared();
			m_overlay_manager->dispose(uids_to_dispose);
		}

		vk::get_resource_manager()->trim();

		vk::reset_global_resources();

		if (ctx->last_frame_sync_time > m_last_heap_sync_time)
		{
			m_last_heap_sync_time = ctx->last_frame_sync_time;

			// Heap cleanup; deallocates memory consumed by the frame if it is still held
			vk::data_heap_manager::restore_snapshot(ctx->heap_snapshot);
		}
	}

	ctx->swap_command_buffer = nullptr;

	// Remove from queued list
	while (!m_queued_frames.empty())
	{
		auto frame = m_queued_frames.front();
		m_queued_frames.pop_front();

		if (frame == ctx)
		{
			break;
		}
	}

	vk::advance_completed_frame_counter();
}
void restart_dma(vk::command_buffer& cmd) {
int device=1; const int* m_device=&device; int m_submit_queue=1;
			vk::fence submit_fence(*m_device);
			vk::queue_submit_t submit_info{ m_submit_queue, &submit_fence };

			cmd.submit(submit_info, VK_TRUE);
			vk::wait_for_fence(&submit_fence, GENERAL_WAIT_TIMEOUT);

			cmd.reset();
			cmd.begin();
}

int checks=0,failures=0;
void check(const std::string& name,bool pass){++checks;if(!pass)++failures;std::cout<<(pass?"PASS ":"FAIL ")<<name<<"\n";}
void reset_test(){
    vk::gc.clear();results.clear();wait_timeouts.clear();completed.clear();
    fence_resets=buffer_resets=buffer_begins=unsafe_resets=unsafe_fence_destruction=0;
    trims=globals_reset=heap_restores=frame_advances=0;poke_status=VK_NOT_READY;
}
void pending(vk::command_buffer_chunk& cmd){cmd.is_pending=true;cmd.eid_tag=41;cmd.reset_id=7;cmd.own_fence.gpu_pending=true;}
bool unlocked(vk::command_buffer_chunk& cmd){return cmd.guard_mutex.readers==0 && !cmd.guard_mutex.writer;}
int main(){
    for(const auto status:{VK_TIMEOUT,VK_ERROR_OUT_OF_HOST_MEMORY,VK_ERROR_OUT_OF_DEVICE_MEMORY,VK_ERROR_DEVICE_LOST,VK_ERROR_UNKNOWN,VK_ERROR_VALIDATION_FAILED}){
        reset_test();vk::command_buffer_chunk cmd;pending(cmd);results={status};
        const auto returned=cmd.wait(37);const auto suffix=" ("+std::to_string(status)+")";
        check("finite wait preserves exact noncompletion result"+suffix,returned==status);
        check("failed wait retains pending event and reuse generation"+suffix,cmd.is_pending && cmd.eid_tag==41 && cmd.reset_id==7);
        check("failed wait does not reset its fence or notify collection"+suffix,fence_resets==0 && completed.empty() && unsafe_resets==0);
        check("failed wait releases the reader guard"+suffix,unlocked(cmd));
        results={VK_SUCCESS};const auto retried=cmd.wait(39);
        check("later completion retires the original event exactly once"+suffix,retried==VK_SUCCESS && !cmd.is_pending && cmd.eid_tag==0 && completed==std::vector<u64>({41}) && fence_resets==1);
        check("completed chunk skips subsequent fence waits"+suffix,cmd.wait(41)==VK_SUCCESS && wait_timeouts==std::vector<u64>({37,39}));
    }
    {
        reset_test();vk::command_buffer_chunk cmd;
        check("idle wait requires no fence or collection operation",cmd.wait(7)==VK_SUCCESS && wait_timeouts.empty() && completed.empty() && fence_resets==0);
        cmd.reset();check("idle reset advances generation and resets only its buffer",cmd.reset_id==1 && buffer_resets==1 && wait_timeouts.empty());
    }
    {
        reset_test();vk::command_buffer_chunk cmd;pending(cmd);results={VK_TIMEOUT,VK_SUCCESS};cmd.reset();
        check("reuse retries an expired bounded wait until completion",wait_timeouts==std::vector<u64>({FRAME_PRESENT_TIMEOUT,0}));
        check("reuse resets the buffer only after confirmed completion",buffer_resets==1 && fence_resets==1 && !cmd.is_pending && cmd.reset_id==8 && unsafe_resets==0 && completed==std::vector<u64>({41}));
    }
    for(const auto status:{VK_ERROR_DEVICE_LOST,VK_ERROR_OUT_OF_HOST_MEMORY}){
        for(const bool timed_out:{false,true}){
            reset_test();vk::command_buffer_chunk cmd;pending(cmd);results=timed_out?std::deque<VkResult>{VK_TIMEOUT,status}:std::deque<VkResult>{status};
            VkResult escaped=VK_SUCCESS;try{cmd.reset();}catch(const api_error& e){escaped=e.result;}
            const auto suffix=" ("+std::to_string(status)+(timed_out?", timeout retry)":", direct)");
            check("reuse propagates the real wait failure"+suffix,escaped==status);
            check("failed reuse retains the pending buffer and resources"+suffix,cmd.is_pending && cmd.eid_tag==41 && cmd.reset_id==7 && buffer_resets==0 && fence_resets==0 && completed.empty() && unlocked(cmd));
        }
    }
    {
        reset_test();vk::command_buffer_chunk cmd;pending(cmd);results={VK_TIMEOUT,VK_SUCCESS};
        vk::frame_context_t frame;frame.swap_command_buffer=&cmd;VKGSRender renderer;renderer.m_queued_frames.push_back(&frame);
        renderer.frame_context_cleanup(&frame);
        check("frame cleanup retries timeout before releasing resources",wait_timeouts==std::vector<u64>({FRAME_PRESENT_TIMEOUT,0}) && completed==std::vector<u64>({41}));
        check("completed frame releases heap and advances once",heap_restores==1 && globals_reset==1 && trims==1 && frame_advances==1 && renderer.m_queued_frames.empty() && !frame.swap_command_buffer && !renderer.swapchain_unavailable);
    }
    for(const auto status:{VK_ERROR_DEVICE_LOST,VK_ERROR_OUT_OF_HOST_MEMORY}){
        for(const bool timed_out:{false,true}){
            reset_test();vk::command_buffer_chunk cmd;pending(cmd);results=timed_out?std::deque<VkResult>{VK_TIMEOUT,status}:std::deque<VkResult>{status};
            vk::frame_context_t frame;frame.swap_command_buffer=&cmd;VKGSRender renderer;renderer.m_queued_frames.push_back(&frame);
            VkResult escaped=VK_SUCCESS;try{renderer.frame_context_cleanup(&frame);}catch(const api_error& e){escaped=e.result;}
            const auto suffix=" ("+std::to_string(status)+(timed_out?", timeout retry)":", direct)");
            check("frame cleanup propagates the real wait failure"+suffix,escaped==status);
            check("failed frame keeps its heap, queue and pending buffer"+suffix,cmd.is_pending && frame.swap_command_buffer==&cmd && renderer.m_queued_frames.size()==1 && !heap_restores && !globals_reset && !trims && !frame_advances && completed.empty());
        }
    }
    {
        reset_test();vk::command_buffer cmd;results={VK_TIMEOUT,VK_SUCCESS};restart_dma(cmd);
        check("DMA restart retries the bounded wait before resetting",wait_timeouts==std::vector<u64>({GENERAL_WAIT_TIMEOUT,0}) && buffer_resets==1 && buffer_begins==1 && !unsafe_resets && !unsafe_fence_destruction && vk::gc.held.empty());
    }
    for(const auto status:{VK_ERROR_DEVICE_LOST,VK_ERROR_OUT_OF_HOST_MEMORY}){
        for(const bool timed_out:{false,true}){
            reset_test();vk::command_buffer cmd;results=timed_out?std::deque<VkResult>{VK_TIMEOUT,status}:std::deque<VkResult>{status};
            VkResult escaped=VK_SUCCESS;try{restart_dma(cmd);}catch(const api_error& e){escaped=e.result;}
            const auto suffix=" ("+std::to_string(status)+(timed_out?", timeout retry)":", direct)");
            check("DMA restart propagates the real wait failure"+suffix,escaped==status);
            check("failed DMA cannot reset or begin its buffer"+suffix,!buffer_resets && !buffer_begins && !unsafe_resets);
            check("failed DMA retains its submitted fence for later collection"+suffix,vk::gc.held.size()==1 && vk::gc.held.front()->gpu_pending && !unsafe_fence_destruction);
        }
    }
    reset_test();
    std::cout<<checks<<" fence completion checks, "<<failures<<" failed\n";
    return failures?1:0;
}
