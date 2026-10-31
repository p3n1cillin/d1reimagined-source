
#include <algorithm>
#include <array>
#include <bit>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <iostream>
#include <mutex>
#include <stdexcept>
#include <string>
#include <vector>
using u64=unsigned long long; using u32=unsigned int; using usz=unsigned int; using uptr=std::uintptr_t;
using VkQueue=u64; using VkCommandBuffer=u64; using VkPipeline=u64; using VkPipelineBindPoint=int;
constexpr u64 VK_NULL_HANDLE=0; constexpr int VK_PIPELINE_BIND_POINT_GRAPHICS=0, VK_PIPELINE_BIND_POINT_COMPUTE=1;
constexpr u32 VK_STRUCTURE_TYPE_CHECKPOINT_DATA_NV=1000206000;
struct VkCheckpointDataNV {u32 sType=0; void* pNext=nullptr; u32 stage=0; void* pCheckpointMarker=nullptr;};
using SetCheckpoint=void(*)(VkCommandBuffer,const void*);
using GetCheckpoints=void(*)(VkQueue,u32*,VkCheckpointDataNV*);
#define ensure(condition) do { if (!(condition)) throw std::runtime_error("invalid checkpoint call"); } while (0)
namespace fmt {
template<class... T> void append(std::string& out,const char* format,T... args) {
    int n=std::snprintf(nullptr,0,format,args...); std::vector<char> buf(n+1);
    std::snprintf(buf.data(),buf.size(),format,args...); out.append(buf.data(),n);
}}
namespace vk {
struct diagnostic_readback_info
	{
		u64 source_image = 0;
		u64 destination_buffer = 0;
		u64 destination_offset = 0;
		u64 destination_length = 0;
		u64 destination_capacity = 0;
		u32 width = 0;
		u32 height = 0;
		u32 pitch = 0;
		u32 format = 0;
		u32 transforms = 0; // bit 0: format conversion, 1: byte swap, 2: tiling
	};
struct diagnostic_checkpoint_context
	{
		u64 graphics_pipeline = 0;
		u64 compute_pipeline = 0;
		u32 graphics_batches = 0;
		u32 compute_dispatches = 0;
		u32 readbacks = 0;
		diagnostic_readback_info readback{};
	};

struct render_device {
    VkQueue m_graphics_queue=1, m_transfer_queue=1;
    SetCheckpoint m_set_checkpoint=nullptr; GetCheckpoints m_get_checkpoints=nullptr;
    bool get_diagnostic_checkpoint_support() const { return m_set_checkpoint && m_get_checkpoints; }
    void record_checkpoint(VkCommandBuffer,const char*,u64,const diagnostic_checkpoint_context& = {}) const;
    std::string retrieve_checkpoint_info() const;
};
struct command_pool {render_device* device; render_device& get_owner() const {return *device;}};
struct command_buffer {
    bool is_open=true; command_pool* pool; VkCommandBuffer commands=10;
    std::array<VkPipeline,2> m_bound_pipelines{0x111,0x222};
    std::array<int,2> m_bound_descriptor_sets{};
    mutable diagnostic_checkpoint_context m_checkpoint_context{};
    void clear_state_cache();
    void checkpoint(const char*,VkPipelineBindPoint=0) const;
    void checkpoint_readback(const char*,const diagnostic_readback_info&) const;
};
struct diagnostic_checkpoint_record
	{
		u64 sequence = 0;
		VkCommandBuffer commands = VK_NULL_HANDLE;
		std::array<char, 48> operation{};
		u64 pipeline = 0;
		diagnostic_checkpoint_context context{};
	};
	static std::array<diagnostic_checkpoint_record, 65536> g_checkpoint_records{};
	static std::mutex g_checkpoint_mutex;
	static u64 g_checkpoint_sequence = 0;

	
void render_device::record_checkpoint(VkCommandBuffer commands, const char* operation, u64 pipeline,
		const diagnostic_checkpoint_context& context) const
	{
		if (!get_diagnostic_checkpoint_support()) return;
		u64 sequence;
		{
			std::lock_guard lock(g_checkpoint_mutex);
			sequence = ++g_checkpoint_sequence;
			auto& record = g_checkpoint_records[sequence % g_checkpoint_records.size()];
			record = {sequence, commands, {}, pipeline, context};
			std::strncpy(record.operation.data(), operation ? operation : "unknown", record.operation.size() - 1);
		}
		m_set_checkpoint(commands, reinterpret_cast<const void*>(static_cast<uptr>(sequence)));
	}
std::string render_device::retrieve_checkpoint_info() const
	{
		// Called only after VK_ERROR_DEVICE_LOST. This API is invalid on a live device.
		if (!get_diagnostic_checkpoint_support()) return {};
		std::string result = "NVIDIA diagnostic checkpoints (bounded command-region hints):\n";
		const std::array<VkQueue, 2> queues{m_graphics_queue, m_transfer_queue};
		for (usz queue_index = 0; queue_index < queues.size(); ++queue_index)
		{
			if (!queues[queue_index] || (queue_index && queues[queue_index] == queues[0])) continue;
			u32 count = 0;
			m_get_checkpoints(queues[queue_index], &count, nullptr);
			const u32 capacity = std::min(count, 64u);
			std::vector<VkCheckpointDataNV> checkpoints(capacity);
			for (auto& checkpoint : checkpoints) checkpoint.sType = VK_STRUCTURE_TYPE_CHECKPOINT_DATA_NV;
			count = capacity;
			if (capacity) m_get_checkpoints(queues[queue_index], &count, checkpoints.data());
			fmt::append(result, "  queue=%u records=%u capacity=%u\n", queue_index, std::min(count, capacity), capacity);
			for (u32 index = 0; index < std::min(count, capacity); ++index)
			{
				const auto& checkpoint = checkpoints[index];
				const u64 sequence = static_cast<u64>(reinterpret_cast<uptr>(checkpoint.pCheckpointMarker));
				std::lock_guard lock(g_checkpoint_mutex);
				const auto& record = g_checkpoint_records[sequence % g_checkpoint_records.size()];
				if (!sequence || record.sequence != sequence)
				{
					fmt::append(result, "    stage=0x%x marker=%llu expired-or-unknown\n", static_cast<u32>(checkpoint.stage), sequence);
					continue;
				}
				fmt::append(result, "    stage=0x%x marker=%llu operation=%s command_buffer=0x%llx pipeline=0x%llx",
					static_cast<u32>(checkpoint.stage), sequence, record.operation.data(),
					std::bit_cast<u64>(record.commands), record.pipeline);
				const auto& context = record.context;
				const auto& readback = context.readback;
				fmt::append(result, " graphics_pipeline=0x%llx compute_pipeline=0x%llx graphics_batches=%u compute_dispatches=%u readbacks=%u",
					context.graphics_pipeline, context.compute_pipeline, context.graphics_batches, context.compute_dispatches, context.readbacks);
				if (context.readbacks)
				{
					fmt::append(result, " readback_image=0x%llx readback_buffer=0x%llx readback_offset=%llu readback_length=%llu readback_capacity=%llu readback_width=%u readback_height=%u readback_pitch=%u readback_format=%u readback_transforms=%u",
						readback.source_image, readback.destination_buffer, readback.destination_offset,
						readback.destination_length, readback.destination_capacity, readback.width, readback.height,
						readback.pitch, readback.format, readback.transforms);
				}
				result += '\n';
			}
		}
		return result;
	}
void command_buffer::clear_state_cache()
	{
		m_checkpoint_context = {};
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
		m_checkpoint_context.graphics_pipeline = std::bit_cast<u64>(m_bound_pipelines[0]);
		m_checkpoint_context.compute_pipeline = std::bit_cast<u64>(m_bound_pipelines[1]);
		if (operation && !std::strcmp(operation, "graphics-draw.before")) ++m_checkpoint_context.graphics_batches;
		if (operation && !std::strcmp(operation, "compute-dispatch.before")) ++m_checkpoint_context.compute_dispatches;
		pool->get_owner().record_checkpoint(commands, operation,
			std::bit_cast<u64>(m_bound_pipelines[static_cast<int>(bind_point)]), m_checkpoint_context);
	}
void command_buffer::checkpoint_readback(const char* operation, const diagnostic_readback_info& readback) const
	{
		if (!pool->get_owner().get_diagnostic_checkpoint_support()) return;
		ensure(is_open);
		m_checkpoint_context.readback = readback;
		if (operation && !std::strcmp(operation, "dma-readback.before")) ++m_checkpoint_context.readbacks;
		checkpoint(operation, VK_PIPELINE_BIND_POINT_COMPUTE);
	}
}
u64 last_marker=0; int set_calls=0, query_calls=0, fill_calls=0;
u32 available=1, returned=1, supplied_capacity=0;
bool initialized=true;
std::array<u64,64> markers{};
void set(VkCommandBuffer,const void* marker) {++set_calls;last_marker=reinterpret_cast<uptr>(marker);}
void query(VkQueue,u32* count,VkCheckpointDataNV* data) {
    ++query_calls;
    if (!data) {*count=available;return;}
    ++fill_calls; supplied_capacity=*count;
    for(u32 i=0;i<*count;++i) initialized&=data[i].sType==VK_STRUCTURE_TYPE_CHECKPOINT_DATA_NV && !data[i].pNext;
    for(u32 i=0;i<std::min(*count,returned);++i) {data[i].stage=0x2000;data[i].pCheckpointMarker=reinterpret_cast<void*>(static_cast<uptr>(markers[i]));}
    *count=returned;
}
void reset_query(u32 count=1,u32 written=1) {available=count;returned=written;query_calls=fill_calls=0;initialized=true;markers.fill(0);}
int checks=0,failures=0;
void check(const char* name,bool pass) {++checks;if(!pass){++failures;std::cerr<<"FAIL "<<name<<"\n";}}
int main() {
    vk::render_device dev; vk::command_pool pool{&dev}; vk::command_buffer cmd{true,&pool};
    cmd.checkpoint("disabled");check("disabled path records no GPU marker",set_calls==0);
    check("disabled path queries no queue",dev.retrieve_checkpoint_info().empty() && query_calls==0);
    dev.m_set_checkpoint=set;check("one missing procedure disables diagnostics",!dev.get_diagnostic_checkpoint_support());
    dev.m_get_checkpoints=query;
    cmd.checkpoint("graphics.before");const auto graphics=last_marker;markers[0]=graphics;
    auto report=dev.retrieve_checkpoint_info();
    check("graphics marker preserves operation and bound pipeline",report.find("operation=graphics.before")!=std::string::npos && report.find("pipeline=0x111")!=std::string::npos);
    check("equal graphics/transfer queue is queried once",query_calls==2 && fill_calls==1);
    check("every output structure is initialized",initialized);
    cmd.checkpoint("compute.before",VK_PIPELINE_BIND_POINT_COMPUTE);reset_query();markers[0]=last_marker;
    report=dev.retrieve_checkpoint_info();
    check("compute uses its independent actual bound pipeline",report.find("pipeline=0x222")!=std::string::npos);
    cmd.checkpoint("graphics-draw.before");cmd.checkpoint("graphics-draw.after");
    cmd.checkpoint("compute-dispatch.before",1);cmd.checkpoint("compute-dispatch.after",1);
    vk::diagnostic_readback_info readback{0x444,0,0,256,0,8,8,32,44,3};
    cmd.checkpoint_readback("dma-readback.before",readback);const auto before=last_marker;
    readback.destination_buffer=0x555;readback.destination_offset=64;readback.destination_capacity=512;
    cmd.checkpoint_readback("dma-readback.mapped",readback);const auto mapped=last_marker;
    cmd.checkpoint_readback("dma-readback.after",readback);
    cmd.m_bound_pipelines[0]=0;cmd.checkpoint("command-buffer.end");const auto end=last_marker;
    reset_query();markers[0]=end;report=dev.retrieve_checkpoint_info();
    check("buffer end retains compute binding even with no graphics pipeline",report.find("graphics_pipeline=0x0 compute_pipeline=0x222")!=std::string::npos);
    check("command region counts before operations once",report.find("graphics_batches=1 compute_dispatches=1 readbacks=1")!=std::string::npos);
    check("buffer end retains mapped DMA range and capacity",report.find("readback_image=0x444 readback_buffer=0x555 readback_offset=64 readback_length=256 readback_capacity=512")!=std::string::npos);
    check("DMA format dimensions and transformations survive buffer end",report.find("readback_width=8 readback_height=8 readback_pitch=32 readback_format=44 readback_transforms=3")!=std::string::npos);
    reset_query();markers[0]=before;report=dev.retrieve_checkpoint_info();
    check("beginning DMA snapshot remains explicitly unmapped",report.find("readback_buffer=0x0 readback_offset=0 readback_length=256 readback_capacity=0")!=std::string::npos);
    readback.destination_buffer=0x666;cmd.m_checkpoint_context.readback=readback;
    reset_query();markers[0]=mapped;
    check("mapped snapshot does not alias later CPU context",dev.retrieve_checkpoint_info().find("readback_buffer=0x555")!=std::string::npos);
    cmd.clear_state_cache();cmd.checkpoint("command-buffer.begin");reset_query();markers[0]=last_marker;
    report=dev.retrieve_checkpoint_info();
    check("new recording clears both pipelines and all diagnostic counts",report.find("graphics_pipeline=0x0 compute_pipeline=0x0 graphics_batches=0 compute_dispatches=0 readbacks=0")!=std::string::npos);
    check("new recording cannot inherit previous readback resources",report.find("readback_image=")==std::string::npos);
    check("new recording clears the stored DMA details",cmd.m_checkpoint_context.readback.source_image==0 && cmd.m_checkpoint_context.readback.destination_buffer==0);
    dev.m_get_checkpoints=nullptr;const auto calls_before=set_calls;
    cmd.checkpoint_readback("dma-readback.before",readback);
    check("disabled readback path neither emits markers nor changes context",set_calls==calls_before && cmd.m_checkpoint_context.readbacks==0 && cmd.m_checkpoint_context.readback.source_image==0);
    dev.m_get_checkpoints=query;
    char name[]="original.operation";dev.record_checkpoint(10,name,0x333);reset_query();markers[0]=last_marker;name[0]='X';
    check("operation name survives caller storage mutation",dev.retrieve_checkpoint_info().find("operation=original.operation")!=std::string::npos);
    dev.record_checkpoint(10,nullptr,0);reset_query();markers[0]=last_marker;
    check("missing operation has a stable label",dev.retrieve_checkpoint_info().find("operation=unknown")!=std::string::npos);
    reset_query(0,0);report=dev.retrieve_checkpoint_info();
    check("zero-count query never calls fill",query_calls==1 && fill_calls==0 && report.find("records=0")!=std::string::npos);
    reset_query(100000,100000);report=dev.retrieve_checkpoint_info();
    check("large driver counts are bounded on allocation and output",supplied_capacity==64 && initialized && report.find("records=64 capacity=64")!=std::string::npos);
    reset_query(2,1);markers[0]=graphics;markers[1]=0xffffffffffffffffull;
    report=dev.retrieve_checkpoint_info();
    check("returned count shrink ignores unwritten entries",report.find("records=1 capacity=2")!=std::string::npos && report.find("expired-or-unknown")==std::string::npos);
    reset_query();markers[0]=0xffffffffffffffffull;
    check("unknown opaque marker is never dereferenced",dev.retrieve_checkpoint_info().find("expired-or-unknown")!=std::string::npos);
    for(u32 i=0;i<65536;++i)dev.record_checkpoint(10,"later",0);
    reset_query();markers[0]=graphics;
    check("expired registry entry is not misattributed",dev.retrieve_checkpoint_info().find("expired-or-unknown")!=std::string::npos);
    dev.m_transfer_queue=2;reset_query();markers[0]=last_marker;dev.retrieve_checkpoint_info();
    check("distinct execution queues are both queried",query_calls==4 && fill_calls==2);
    dev.m_transfer_queue=0;reset_query();dev.retrieve_checkpoint_info();
    check("null queue is excluded",query_calls==2);
    bool refused=false;try{cmd.checkpoint("invalid",2);}catch(const std::runtime_error&){refused=true;}
    check("unsupported pipeline bind point is refused",refused);
    refused=false;try{cmd.checkpoint("negative",-1);}catch(const std::runtime_error&){refused=true;}
    check("negative bind point never indexes outside pipeline state",refused);
    refused=false;cmd.is_open=false;try{cmd.checkpoint("closed");}catch(const std::runtime_error&){refused=true;}
    check("closed command buffer is refused",refused);
    refused=false;try{cmd.checkpoint_readback("dma-readback.before",readback);}catch(const std::runtime_error&){refused=true;}
    check("closed readback cannot mutate diagnostic context",refused && cmd.m_checkpoint_context.readbacks==0);
    std::cout<<checks<<" checkpoint checks, "<<failures<<" failed\n";return failures?1:0;
}
