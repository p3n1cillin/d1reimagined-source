#include "commands.h"
#include "device.h"
#include "shared.h"
#include "sync.h"
#include "util/logs.hpp"
#include <bit>

namespace vk
{
	// This queue flushing method to be implemented by the backend as behavior depends on config
	void queue_submit(const queue_submit_t& submit_info, VkBool32 flush);

	void command_pool::create(vk::render_device& dev, u32 queue_family_id)
	{
		owner = &dev;
		queue_family = queue_family_id;

		VkCommandPoolCreateInfo infos = {};
		infos.flags            = VK_COMMAND_POOL_CREATE_TRANSIENT_BIT | VK_COMMAND_POOL_CREATE_RESET_COMMAND_BUFFER_BIT;
		infos.sType            = VK_STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO;
		infos.queueFamilyIndex = queue_family;

		CHECK_RESULT(vkCreateCommandPool(dev, &infos, nullptr, &pool));
	}

	void command_pool::destroy()
	{
		if (!pool)
			return;

		vkDestroyCommandPool((*owner), pool, nullptr);
		pool = nullptr;
	}

	vk::render_device& command_pool::get_owner() const
	{
		return (*owner);
	}

	u32 command_pool::get_queue_family() const
	{
		return queue_family;
	}

	command_pool::operator VkCommandPool() const
	{
		return pool;
	}

	void command_buffer::create(command_pool& cmd_pool)
	{
		VkCommandBufferAllocateInfo infos = {};
		infos.sType = VK_STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO;
		infos.commandBufferCount = 1;
		infos.commandPool = +cmd_pool;
		infos.level = VK_COMMAND_BUFFER_LEVEL_PRIMARY;
		CHECK_RESULT(vkAllocateCommandBuffers(cmd_pool.get_owner(), &infos, &commands));

		m_submit_fence = new fence(cmd_pool.get_owner());
		pool = &cmd_pool;
	}

	void command_buffer::destroy()
	{
		vkFreeCommandBuffers(pool->get_owner(), (*pool), 1, &commands);

		if (m_submit_fence)
		{
			//vkDestroyFence(pool->get_owner(), m_submit_fence, nullptr);
			delete m_submit_fence;
			m_submit_fence = nullptr;
		}
	}

	void command_buffer::reset()
	{
		// Do the driver reset
		CHECK_RESULT(vkResetCommandBuffer(commands, 0));
	}

	void command_buffer::clear_state_cache()
	{
		m_bound_pipelines[0] = VK_NULL_HANDLE;
		m_bound_pipelines[1] = VK_NULL_HANDLE;
		for (auto& bindings : m_bound_descriptor_sets)
		{
			bindings = {};
		}
	}

	void command_buffer::checkpoint(const char* operation, VkPipelineBindPoint bind_point) const
	{
		if (!pool->get_owner().get_diagnostic_checkpoint_support()) return;
		ensure(is_open && (bind_point == VK_PIPELINE_BIND_POINT_GRAPHICS || bind_point == VK_PIPELINE_BIND_POINT_COMPUTE));
		pool->get_owner().record_checkpoint(commands, operation,
			std::bit_cast<u64>(m_bound_pipelines[static_cast<int>(bind_point)]));
	}

	void command_buffer::begin()
	{
		if (m_submit_fence && is_pending)
		{
			wait_for_fence(m_submit_fence);
			is_pending = false;

			//CHECK_RESULT(vkResetFences(pool->get_owner(), 1, &m_submit_fence));
			m_submit_fence->reset();
			reset();
		}

		if (is_open)
			return;

		VkCommandBufferInheritanceInfo inheritance_info = {};
		inheritance_info.sType = VK_STRUCTURE_TYPE_COMMAND_BUFFER_INHERITANCE_INFO;

		VkCommandBufferBeginInfo begin_infos = {};
		begin_infos.pInheritanceInfo = &inheritance_info;
		begin_infos.sType = VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO;
		begin_infos.flags = VK_COMMAND_BUFFER_USAGE_ONE_TIME_SUBMIT_BIT;
		CHECK_RESULT(vkBeginCommandBuffer(commands, &begin_infos));
		is_open = true;

		clear_state_cache();
		checkpoint("command-buffer.begin");
	}

	void command_buffer::end()
	{
		if (!is_open)
		{
			rsx_log.error("commandbuffer->end was called but commandbuffer is not in a recording state");
			return;
		}

		checkpoint("command-buffer.end");
		CHECK_RESULT(vkEndCommandBuffer(commands));
		is_open = false;
	}

	void command_buffer::submit(queue_submit_t& submit_info, VkBool32 flush)
	{
		if (is_open)
		{
			rsx_log.error("commandbuffer->submit was called whilst the command buffer is in a recording state");
			return;
		}

		// Check for hanging queries to avoid driver hang
		ensure((flags & cb_has_open_query) == 0); // "close and submit of commandbuffer with a hanging query!"

		if (!submit_info.pfence)
		{
			submit_info.pfence = m_submit_fence;
			is_pending = bool(submit_info.pfence);
		}

		submit_info.commands = this->commands;
		queue_submit(submit_info, flush);
		clear_flags();
	}

	void command_buffer::bind_pipeline(VkPipeline pipeline, VkPipelineBindPoint bind_point) const
	{
		ensure(is_open && bind_point <= VK_PIPELINE_BIND_POINT_COMPUTE);
		auto& cached = m_bound_pipelines[static_cast<int>(bind_point)];
		if (cached == pipeline)
		{
			return;
		}

		cached = pipeline;
		vkCmdBindPipeline(commands, bind_point, pipeline);
	}

	void command_buffer::bind_descriptor_sets(
		const std::span<VkDescriptorSet>& sets,
		const std::span<u32>& dynamic_offsets,
		VkPipelineBindPoint bind_point,
		VkPipelineLayout pipe_layout) const
	{
		const u32 num_sets = ::size32(sets);
		ensure(is_open && num_sets > 0 && num_sets <= 2 && bind_point <= VK_PIPELINE_BIND_POINT_COMPUTE);

		if (dynamic_offsets.empty())
		{
			bind_descriptor_sets(sets, bind_point, pipe_layout);
			return;
		}

		// Dynamic offsets are consumed by this call. Do not let a subsequent
		// static bind inherit their cache entry, even when the handles match.
		m_bound_descriptor_sets[static_cast<int>(bind_point)] = {};
		vkCmdBindDescriptorSets(commands, bind_point, pipe_layout, 0, num_sets, sets.data(), ::size32(dynamic_offsets), dynamic_offsets.data());
	}

	void command_buffer::bind_descriptor_sets(
		const std::span<VkDescriptorSet>& sets,
		VkPipelineBindPoint bind_point,
		VkPipelineLayout pipe_layout) const
	{
		const u32 num_sets = ::size32(sets);
		ensure(is_open && num_sets > 0 && num_sets <= 2 && bind_point <= VK_PIPELINE_BIND_POINT_COMPUTE);
		auto& cached = m_bound_descriptor_sets[static_cast<int>(bind_point)];

		if (cached.layout == pipe_layout && cached.count == num_sets &&
			!memcmp(sets.data(), cached.sets.data(), sets.size_bytes()))
		{
			return;
		}

		// Layout changes can disturb higher-numbered sets. Cache only the
		// complete binding operation, independently for graphics and compute.
		cached.sets.fill(VK_NULL_HANDLE);
		std::memcpy(cached.sets.data(), sets.data(), sets.size_bytes());
		cached.layout = pipe_layout;
		cached.count = num_sets;
		vkCmdBindDescriptorSets(commands, bind_point, pipe_layout, 0, num_sets, sets.data(), 0, nullptr);
	}
}
