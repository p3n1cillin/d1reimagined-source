
#include <array>
#include <atomic>
#include <cstdint>
#include <iostream>
#include <mutex>
#include <stdexcept>
#include <string>
#include <thread>
using u32 = std::uint32_t;
using VkQueue = std::uint64_t;
using VkSemaphore = std::uint64_t;
using VkCommandBuffer = std::uint64_t;
using VkSwapchainKHR = std::uint64_t;
using VkPipelineStageFlags = u32;
using VkResult = int;
constexpr int VK_SUCCESS=0, VK_ERROR_OUT_OF_HOST_MEMORY=-1, VK_ERROR_OUT_OF_DEVICE_MEMORY=-2;
constexpr int VK_ERROR_DEVICE_LOST=-4, VK_SUBOPTIMAL_KHR=1000001003, VK_ERROR_OUT_OF_DATE_KHR=-1000001004;
constexpr int VK_STRUCTURE_TYPE_SUBMIT_INFO=4, VK_STRUCTURE_TYPE_PRESENT_INFO_KHR=1000001001;
constexpr VkSemaphore VK_NULL_HANDLE=0;
thread_local int lock_depth=0;
struct tracked_mutex {
    std::mutex native;
    void lock() { native.lock(); ++lock_depth; }
    void unlock() { --lock_depth; native.unlock(); }
    bool try_lock() { if (!native.try_lock()) return false; ++lock_depth; return true; }
} g_submit_mutex;
void acquire_global_submit_lock() { g_submit_mutex.lock(); }
void release_global_submit_lock() { g_submit_mutex.unlock(); }
struct vk_failure : std::runtime_error {
    VkResult result;
    explicit vk_failure(VkResult value):std::runtime_error("Vulkan failure"),result(value){}
};
struct driver_failure : std::runtime_error { driver_failure():std::runtime_error("driver exception"){} };
bool error_checked_unlocked=true;
void check_result(VkResult result) {
    error_checked_unlocked = lock_depth==0;
    if (result!=VK_SUCCESS) throw vk_failure(result);
}
#define CHECK_RESULT(result) check_result(result)
#define ensure(value) do { if (!(value)) throw std::runtime_error("ensure"); } while(false)
struct VkSubmitInfo {
    int sType; const void* pNext; u32 waitSemaphoreCount; const VkSemaphore* pWaitSemaphores;
    const VkPipelineStageFlags* pWaitDstStageMask; u32 commandBufferCount;
    const VkCommandBuffer* pCommandBuffers; u32 signalSemaphoreCount; const VkSemaphore* pSignalSemaphores;
};
struct VkPresentInfoKHR {
    int sType{}; const void* pNext{}; u32 waitSemaphoreCount{}; const VkSemaphore* pWaitSemaphores{};
    u32 swapchainCount{}; const VkSwapchainKHR* pSwapchains{}; const u32* pImageIndices{};
};
using PFN_vkQueuePresentKHR = VkResult(*)(VkQueue,const VkPresentInfoKHR*);
int submit_calls=0, present_calls=0, flushed=0;
bool submit_locked=false, present_locked=false, competing_submit_blocked=false;
bool submit_batch_matches=false, present_batch_matches=false, signal_after_driver=false;
bool driver_throws=false;
VkResult submit_result=VK_SUCCESS, present_result=VK_SUCCESS;
struct fence {
    std::uint64_t handle=77;
    void signal_flushed() { ++flushed; signal_after_driver=submit_calls==1 && lock_depth==0; }
};
struct queue_submit_t {
    VkQueue queue=41; fence* pfence{}; VkCommandBuffer commands=57;
    u32 wait_semaphores_count=2, signal_semaphores_count=2;
    std::array<VkSemaphore,2> wait_semaphores{11,12}, signal_semaphores{21,22};
    std::array<VkPipelineStageFlags,2> wait_stages{8,16};
};
const queue_submit_t* expected_submit=nullptr;
VkSemaphore expected_semaphore=0; u32 expected_image=0;
bool competing_lock_is_blocked() {
    bool blocked=false;
    std::thread worker([&] {
        const bool acquired=g_submit_mutex.try_lock();
        blocked=!acquired;
        if (acquired) g_submit_mutex.unlock();
    });
    worker.join();
    return blocked;
}
VkResult vkQueueSubmit(VkQueue queue,u32 count,const VkSubmitInfo* info,std::uint64_t handle) {
    ++submit_calls; submit_locked=lock_depth==1;
    const auto& expected=*expected_submit;
    submit_batch_matches=queue==expected.queue && count==1 && handle==expected.pfence->handle &&
        info->sType==VK_STRUCTURE_TYPE_SUBMIT_INFO && !info->pNext &&
        info->waitSemaphoreCount==expected.wait_semaphores_count &&
        info->pWaitSemaphores==expected.wait_semaphores.data() &&
        info->pWaitDstStageMask==expected.wait_stages.data() && info->commandBufferCount==1 &&
        info->pCommandBuffers==&expected.commands &&
        info->signalSemaphoreCount==expected.signal_semaphores_count &&
        info->pSignalSemaphores==expected.signal_semaphores.data();
    if (driver_throws) throw driver_failure();
    return submit_result;
}
VkResult fake_present(VkQueue queue,const VkPresentInfoKHR* info) {
    ++present_calls; present_locked=lock_depth==1;
    competing_submit_blocked=competing_lock_is_blocked();
    present_batch_matches=queue==41 && info->sType==VK_STRUCTURE_TYPE_PRESENT_INFO_KHR &&
        !info->pNext && info->swapchainCount==1 && *info->pSwapchains==91 &&
        *info->pImageIndices==expected_image && info->waitSemaphoreCount==(expected_semaphore?1:0) &&
        (expected_semaphore ? info->pWaitSemaphores && *info->pWaitSemaphores==expected_semaphore : !info->pWaitSemaphores);
    if (driver_throws) throw driver_failure();
    return present_result;
}
struct device { VkQueue get_present_queue() const { return 41; } };
namespace vk {
struct swapchain_WSI {
    device dev;
    VkSwapchainKHR m_vk_swapchain=91;
    PFN_vkQueuePresentKHR _vkQueuePresentKHR=fake_present;
    VkResult present(VkSemaphore,u32);
};

static void queue_submit_impl(const queue_submit_t& submit_info)
{
		ensure(submit_info.pfence);
		acquire_global_submit_lock();
		VkSubmitInfo info
		{
			.sType = VK_STRUCTURE_TYPE_SUBMIT_INFO,
			.pNext = nullptr,
			.waitSemaphoreCount = submit_info.wait_semaphores_count,
			.pWaitSemaphores = submit_info.wait_semaphores.data(),
			.pWaitDstStageMask = submit_info.wait_stages.data(),
			.commandBufferCount = 1,
			.pCommandBuffers = &submit_info.commands,
			.signalSemaphoreCount = submit_info.signal_semaphores_count,
			.pSignalSemaphores = submit_info.signal_semaphores.data()
		};

		vkQueueSubmit(submit_info.queue, 1, &info, submit_info.pfence->handle);
		release_global_submit_lock();

		// Signal fence
		submit_info.pfence->signal_flushed();
	}
VkResult swapchain_WSI::present(VkSemaphore semaphore, u32 image)
{
		VkPresentInfoKHR present = {};
		present.sType = VK_STRUCTURE_TYPE_PRESENT_INFO_KHR;
		present.pNext = nullptr;
		present.swapchainCount = 1;
		present.pSwapchains = &m_vk_swapchain;
		present.pImageIndices = &image;

		if (semaphore != VK_NULL_HANDLE)
		{
			present.waitSemaphoreCount = 1;
			present.pWaitSemaphores = &semaphore;
		}

		return _vkQueuePresentKHR(dev.get_present_queue(), &present);
	}
} // namespace vk
int checks=0, failures=0;
void check(const std::string& name,bool pass) {
    ++checks; if (!pass) ++failures;
    std::cout<<(pass?"PASS ":"FAIL ")<<name<<"\n";
}
void reset() {
    // A baseline exception can leak the same-thread lock. Record that failure
    // before releasing it so subsequent independent tests can still execute.
    while(lock_depth) g_submit_mutex.unlock();
    submit_calls=present_calls=flushed=0; driver_throws=false;
    submit_locked=present_locked=competing_submit_blocked=false;
    submit_batch_matches=present_batch_matches=signal_after_driver=false;
    error_checked_unlocked=true; submit_result=present_result=VK_SUCCESS;
}
int main() {
    fence target; queue_submit_t packet; packet.pfence=&target; expected_submit=&packet;
    vk::swapchain_WSI swapchain;
    for (const u32 count:{0u,2u}) {
        reset(); packet.wait_semaphores_count=packet.signal_semaphores_count=count;
        vk::queue_submit_impl(packet);
        check("submission preserves its complete batch",submit_batch_matches);
        check("submission owns the queue lock",submit_locked);
        check("successful submission flushes exactly once after driver return",flushed==1 && signal_after_driver);
        check("successful submission releases the queue lock",lock_depth==0 && !competing_lock_is_blocked());
    }
    for (const auto status:{VK_ERROR_OUT_OF_HOST_MEMORY,VK_ERROR_OUT_OF_DEVICE_MEMORY,VK_ERROR_DEVICE_LOST}) {
        reset(); submit_result=status; int caught=0;
        try { vk::queue_submit_impl(packet); } catch(const vk_failure& error) { caught=error.result; }
        const std::string suffix=" ("+std::to_string(status)+")";
        check("submission propagates exact API failure"+suffix,caught==status);
        check("failed submission does not flush the fence"+suffix,flushed==0);
        check("failure handler runs after releasing queue lock"+suffix,error_checked_unlocked && lock_depth==0);
        check("independent submission can follow a failure"+suffix,!competing_lock_is_blocked());
    }
    reset(); packet.pfence=nullptr; bool refused=false;
    try { vk::queue_submit_impl(packet); } catch(const std::runtime_error&) { refused=true; }
    check("null fence is refused before any API call or lock",refused && !submit_calls && !flushed && lock_depth==0);
    packet.pfence=&target;
    reset(); driver_throws=true; bool escaped=false;
    try { vk::queue_submit_impl(packet); } catch(const driver_failure&) { escaped=true; }
    check("submission exception preserves failure and fence state",escaped && flushed==0);
    check("submission exception releases queue lock",lock_depth==0);
    for (const auto status:{VK_SUCCESS,VK_SUBOPTIMAL_KHR,VK_ERROR_OUT_OF_DATE_KHR,VK_ERROR_DEVICE_LOST}) {
        for (const VkSemaphore semaphore:{VkSemaphore{0},VkSemaphore{33}}) {
            reset(); expected_semaphore=semaphore; expected_image=3; present_result=status;
            const auto actual=swapchain.present(semaphore,expected_image);
            check("presentation preserves result and semaphore/image batch",actual==status && present_batch_matches && present_calls==1);
            check("presentation prevents concurrent queue submission",present_locked && competing_submit_blocked);
            check("presentation releases lock on every returned result",lock_depth==0 && !competing_lock_is_blocked());
        }
    }
    reset(); driver_throws=true; escaped=false;
    try { swapchain.present(0,0); } catch(const driver_failure&) { escaped=true; }
    check("presentation exception releases queue lock",escaped && lock_depth==0);
    std::cout<<checks<<" queue synchronization checks, "<<failures<<" failed\n";
    return failures?1:0;
}
