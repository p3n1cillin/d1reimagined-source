
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
struct render_device {
    VkQueue m_graphics_queue=1, m_transfer_queue=1;
    SetCheckpoint m_set_checkpoint=nullptr; GetCheckpoints m_get_checkpoints=nullptr;
    bool get_diagnostic_checkpoint_support() const { return m_set_checkpoint && m_get_checkpoints; }
    void record_checkpoint(VkCommandBuffer,const char*,u64) const;
    std::string retrieve_checkpoint_info() const;
};
struct command_pool {render_device* device; render_device& get_owner() const {return *device;}};
struct command_buffer {
    bool is_open=true; command_pool* pool; VkCommandBuffer commands=10;
    std::array<VkPipeline,2> m_bound_pipelines{0x111,0x222};
    void checkpoint(const char*,VkPipelineBindPoint=0) const;
};
struct diagnostic_checkpoint_record
	{
		u64 sequence = 0;
		VkCommandBuffer commands = VK_NULL_HANDLE;
		std::array<char, 48> operation{};
		u64 pipeline = 0;
	};
	static std::array<diagnostic_checkpoint_record, 65536> g_checkpoint_records{};
	static std::mutex g_checkpoint_mutex;
	static u64 g_checkpoint_sequence = 0;

	
void render_device::record_checkpoint(VkCommandBuffer commands, const char* operation, u64 pipeline) const
	{
		if (!get_diagnostic_checkpoint_support()) return;
		u64 sequence;
		{
			std::lock_guard lock(g_checkpoint_mutex);
			sequence = ++g_checkpoint_sequence;
			auto& record = g_checkpoint_records[sequence % g_checkpoint_records.size()];
			record = {sequence, commands, {}, pipeline};
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
				fmt::append(result, "    stage=0x%x marker=%llu operation=%s command_buffer=0x%llx pipeline=0x%llx\n",
					static_cast<u32>(checkpoint.stage), sequence, record.operation.data(),
					std::bit_cast<u64>(record.commands), record.pipeline);
			}
		}
		return result;
	}
void command_buffer::checkpoint(const char* operation, VkPipelineBindPoint bind_point) const
	{
		if (!pool->get_owner().get_diagnostic_checkpoint_support()) return;
		ensure(is_open && bind_point <= VK_PIPELINE_BIND_POINT_COMPUTE);
		pool->get_owner().record_checkpoint(commands, operation,
			std::bit_cast<u64>(m_bound_pipelines[static_cast<int>(bind_point)]));
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
    refused=false;cmd.is_open=false;try{cmd.checkpoint("closed");}catch(const std::runtime_error&){refused=true;}
    check("closed command buffer is refused",refused);
    std::cout<<checks<<" checkpoint checks, "<<failures<<" failed\n";return failures?1:0;
}
