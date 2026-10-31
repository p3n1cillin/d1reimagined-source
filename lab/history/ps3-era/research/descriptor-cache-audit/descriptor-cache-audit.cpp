
#include <algorithm>
#include <array>
#include <cstdint>
#include <cstring>
#include <iostream>
#include <span>
#include <stdexcept>
#include <vector>
using u32 = std::uint32_t;
using VkDescriptorSet = std::uint64_t; using VkPipeline = std::uint64_t;
using VkPipelineLayout = std::uint64_t; using VkCommandBuffer = std::uint64_t;
enum VkPipelineBindPoint { VK_PIPELINE_BIND_POINT_GRAPHICS, VK_PIPELINE_BIND_POINT_COMPUTE };
constexpr std::uint64_t VK_NULL_HANDLE = 0;
void ensure(bool ok) { if (!ok) throw std::runtime_error("assertion"); }
template<class T> u32 size32(const T& t) { return static_cast<u32>(t.size()); }
struct binding { std::array<VkDescriptorSet,2> sets{}; VkPipelineLayout layout=0; std::vector<u32> offsets; };
std::array<binding,2> actual;
unsigned calls=0;
void vkCmdBindDescriptorSets(VkCommandBuffer, VkPipelineBindPoint point, VkPipelineLayout layout,
                            u32 first, u32 count, const VkDescriptorSet* sets, u32 offset_count, const u32* offsets) {
    ensure(first == 0 && count <= 2); ++calls;
    // A different layout conservatively disturbs earlier bindings. No Vulkan execution is simulated.
    auto& state=actual[point];
    if (layout != state.layout) state.sets.fill(0);
    state.layout=layout;
    std::copy_n(sets,count,state.sets.begin());
    state.offsets.clear();
    if (offset_count) state.offsets.assign(offsets,offsets+offset_count);
}
namespace vk {
struct command_buffer {
    bool is_open=true; VkCommandBuffer commands=1;

		struct descriptor_binding_state
		{
			std::array<VkDescriptorSet, 2> sets{{ VK_NULL_HANDLE }};
			VkPipelineLayout layout = VK_NULL_HANDLE;
			u32 count = 0;
		};
		mutable std::array<descriptor_binding_state, 2> m_bound_descriptor_sets{};
		mutable std::array<VkPipeline, 2> m_bound_pipelines{{ VK_NULL_HANDLE }};

		
    void clear_state_cache();
    void bind_descriptor_sets(const std::span<VkDescriptorSet>&, VkPipelineBindPoint, VkPipelineLayout) const;
    void bind_descriptor_sets(const std::span<VkDescriptorSet>&, const std::span<u32>&, VkPipelineBindPoint, VkPipelineLayout) const;
};
void command_buffer::clear_state_cache()
	{
		m_bound_pipelines[0] = VK_NULL_HANDLE;
		m_bound_pipelines[1] = VK_NULL_HANDLE;
		for (auto& bindings : m_bound_descriptor_sets)
		{
			bindings = {};
		}
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
int main() {
    int checks=0, failures=0;
    auto check=[&](const char* name,bool ok) { ++checks; if(!ok) { ++failures; std::cout<<"FAIL "<<name<<'\n'; } };
    const auto graphics=VK_PIPELINE_BIND_POINT_GRAPHICS, compute=VK_PIPELINE_BIND_POINT_COMPUTE;
    std::array<VkDescriptorSet,2> sets{11,12};
    std::span<VkDescriptorSet> both(sets), one(sets.data(),1);
    std::array<u32,1> offsets{256}; std::span<u32> dynamic(offsets), empty;
    vk::command_buffer cmd;
    cmd.bind_descriptor_sets(both,graphics,100);
    check("initial bind reaches API",calls==1 && actual[graphics].sets==sets);
    cmd.bind_descriptor_sets(both,graphics,100);
    check("identical static binding stays cached",calls==1);
    cmd.bind_descriptor_sets(both,compute,100);
    check("compute receives its independent binding",actual[compute].sets==sets && actual[compute].layout==100);
    const auto before_graphics=calls;
    cmd.bind_descriptor_sets(both,graphics,100);
    check("compute binding preserves graphics cache",calls==before_graphics && actual[graphics].sets==sets);
    cmd.bind_descriptor_sets(both,graphics,200);
    check("changed pipeline layout reaches API",actual[graphics].layout==200);
    cmd.bind_descriptor_sets(one,graphics,300);
    check("single-set different layout reaches API",actual[graphics].layout==300);
    cmd.bind_descriptor_sets(both,graphics,200);
    check("higher set is rebound after layout disturbance",actual[graphics].layout==200 && actual[graphics].sets==sets);
    cmd.bind_descriptor_sets(both,dynamic,graphics,200);
    check("dynamic offsets reach API",actual[graphics].offsets==std::vector<u32>{256});
    const auto before_dynamic=calls;
    cmd.bind_descriptor_sets(both,dynamic,graphics,200);
    check("dynamic binding is not elided",calls==before_dynamic+1);
    cmd.bind_descriptor_sets(both,graphics,200);
    check("static bind after dynamic call reaches API",actual[graphics].offsets.empty());
    const auto before_empty=calls;
    cmd.bind_descriptor_sets(both,empty,graphics,200);
    check("empty dynamic overload shares static cache",calls==before_empty);
    cmd.clear_state_cache(); actual={};
    cmd.bind_descriptor_sets(both,graphics,200);
    check("reset forces graphics rebind",actual[graphics].sets==sets && actual[graphics].layout==200);
    cmd.bind_descriptor_sets(both,compute,100);
    check("reset forces compute rebind",actual[compute].sets==sets && actual[compute].layout==100);
    const auto final_calls=calls;
    cmd.bind_descriptor_sets(both,empty,compute,100);
    check("empty overload caches independent compute binding",calls==final_calls);
    std::cout<<checks<<" descriptor binding checks, "<<failures<<" failed\n";
    return failures ? 1 : 0;
}
