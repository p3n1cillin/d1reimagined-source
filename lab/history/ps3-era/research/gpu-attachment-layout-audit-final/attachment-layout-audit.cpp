
#include <algorithm>
#include <cstdint>
#include <iostream>
#include <map>
#include <numeric>
#include <stdexcept>
#include <string>
#include <vector>
using u8=std::uint8_t; using u32=std::uint32_t; using u64=std::uint64_t; using usz=std::size_t; using s32=std::int32_t;
using VkImageLayout=int; using VkFormat=int; using VkSampleCountFlagBits=int;
using VkRenderPass=u64; using VkFramebuffer=u64; using VkPipelineStageFlags=u32; using VkAccessFlags=u32;
constexpr int VK_NULL_HANDLE=0, VK_IMAGE_LAYOUT_GENERAL=1, VK_IMAGE_LAYOUT_COLOR_ATTACHMENT_OPTIMAL=2;
constexpr int VK_IMAGE_LAYOUT_DEPTH_STENCIL_ATTACHMENT_OPTIMAL=3, VK_IMAGE_LAYOUT_ATTACHMENT_FEEDBACK_LOOP_OPTIMAL_EXT=100;
constexpr int VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL=5, VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL=6, VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL=7;
constexpr int VK_FORMAT_D16_UNORM=10, VK_FORMAT_D32_SFLOAT=11, VK_FORMAT_D24_UNORM_S8_UINT=12, VK_FORMAT_D32_SFLOAT_S8_UINT=13;
constexpr u32 VK_IMAGE_ASPECT_COLOR_BIT=1, VK_IMAGE_ASPECT_DEPTH_BIT=2;
constexpr u32 VK_PIPELINE_STAGE_COLOR_ATTACHMENT_OUTPUT_BIT=1, VK_PIPELINE_STAGE_FRAGMENT_SHADER_BIT=2, VK_PIPELINE_STAGE_VERTEX_SHADER_BIT=4;
constexpr u32 VK_PIPELINE_STAGE_EARLY_FRAGMENT_TESTS_BIT=8, VK_PIPELINE_STAGE_LATE_FRAGMENT_TESTS_BIT=16;
constexpr u32 VK_ACCESS_COLOR_ATTACHMENT_WRITE_BIT=1, VK_ACCESS_SHADER_READ_BIT=2, VK_ACCESS_COLOR_ATTACHMENT_READ_BIT=4;
constexpr u32 VK_ACCESS_DEPTH_STENCIL_ATTACHMENT_WRITE_BIT=8, VK_ACCESS_DEPTH_STENCIL_ATTACHMENT_READ_BIT=16;
constexpr u32 RSX_SHADER_CONTROL_PROGRAMMABLE_BLENDING=1, RSX_SHADER_CONTROL_EMULATE_DEPTH_COMPARE=2;
constexpr int VK_STRUCTURE_TYPE_RENDER_PASS_BEGIN_INFO=1, VK_SUBPASS_CONTENTS_INLINE=0;
template<class T, class... Args> T ensure(T value, Args...) { if (!value) throw std::runtime_error("ensure"); return value; }
template<class T> u32 size32(const T& value) { return static_cast<u32>(value.size()); }
namespace fmt { template<class... Args> [[noreturn]] void throw_exception(Args...) { throw std::runtime_error("invalid key"); } }
struct positionu { u32 x,y; }; struct sizeu { u32 width,height; }; struct coordu { u32 x,y,width,height; coordu(positionu p,sizeu s):x(p.x),y(p.y),width(s.width),height(s.height){} };
struct VkAttachmentReference { u32 attachment; VkImageLayout layout; };
struct range { u32 aspect,base_mip,levels,base_layer,layers; };
struct VkRenderPassBeginInfo { int sType{}; VkRenderPass renderPass{}; VkFramebuffer framebuffer{}; struct { struct {s32 x{},y{};} offset; struct {u32 width{},height{};} extent; } renderArea; };
namespace rsx {
enum class texture_upload_context { framebuffer_storage, blit_engine_src };
struct sampled_image_descriptor_base { texture_upload_context upload_context=texture_upload_context::framebuffer_storage; bool is_cyclic_reference=true; };
constexpr int default_remap_vector=0;
}
namespace vk {
struct command_buffer { int id=1; operator int() const { return id; } };
void end_renderpass(const command_buffer&);
struct image_view;
struct image {
    VkImageLayout current_layout=VK_IMAGE_LAYOUT_DEPTH_STENCIL_ATTACHMENT_OPTIMAL;
    u64 value=1; u32 aspects=VK_IMAGE_ASPECT_DEPTH_BIT; bool feedback=true; int barriers=0;
    image_view* view=nullptr;
    u32 aspect() const { return aspects; }
    void change_layout(command_buffer& cmd, VkImageLayout layout) { if(current_layout!=layout) end_renderpass(cmd); current_layout=layout; }
    void texture_barrier(command_buffer& cmd) { end_renderpass(cmd); ++barriers; current_layout=feedback?VK_IMAGE_LAYOUT_ATTACHMENT_FEEDBACK_LOOP_OPTIMAL_EXT:VK_IMAGE_LAYOUT_GENERAL; }
    image_view* get_view(int,u32) { return view; }
};
struct image_view { vk::image* raw; vk::image* image() const { return raw; } };
image* as_rtt(image* raw) { return raw; }
int null_sampler() { return 0; }
constexpr int glsl_binding_set=1;
namespace glsl { constexpr int binding_set_index_fragment=glsl_binding_set; }
void insert_image_memory_barrier(command_buffer& cmd,u64,VkImageLayout,VkImageLayout,VkPipelineStageFlags,VkPipelineStageFlags,VkAccessFlags,VkAccessFlags,range) { end_renderpass(cmd); }
union renderpass_key_blob
{
	private:
		// Internal utils
		static u64 encode_layout(VkImageLayout layout)
		{
			switch (+layout)
			{
			case VK_IMAGE_LAYOUT_GENERAL:
			case VK_IMAGE_LAYOUT_COLOR_ATTACHMENT_OPTIMAL:
			case VK_IMAGE_LAYOUT_DEPTH_STENCIL_ATTACHMENT_OPTIMAL:
				return static_cast<u64>(layout);
			case VK_IMAGE_LAYOUT_ATTACHMENT_FEEDBACK_LOOP_OPTIMAL_EXT:
				return 4ull;
			default:
				fmt::throw_exception("Unsupported layout 0x%llx here", static_cast<usz>(layout));
			}
		}

		static VkImageLayout decode_layout(u64 encoded)
		{
			switch (encoded)
			{
			case 1:
			case 2:
			case 3:
				return static_cast<VkImageLayout>(encoded);
			case 4:
				return VK_IMAGE_LAYOUT_ATTACHMENT_FEEDBACK_LOOP_OPTIMAL_EXT;
			default:
				fmt::throw_exception("Unsupported layout encoding 0x%llx here", encoded);
			}
		}

	public:
		u64 encoded;

		struct
		{
			u64 color_format  : 8;
			u64 depth_format  : 8;
			u64 sample_count  : 6;
			u64 layout_blob   : 15;
			u64 input_attachments_mask : 5;
		};

		renderpass_key_blob(u64 encoded_) : encoded(encoded_)
		{}

		// Encoders
		inline void set_layout(u32 index, VkImageLayout layout)
		{
			switch (+layout)
			{
			case VK_IMAGE_LAYOUT_ATTACHMENT_FEEDBACK_LOOP_OPTIMAL_EXT:
			case VK_IMAGE_LAYOUT_DEPTH_STENCIL_ATTACHMENT_OPTIMAL:
			case VK_IMAGE_LAYOUT_COLOR_ATTACHMENT_OPTIMAL:
			case VK_IMAGE_LAYOUT_GENERAL:
				layout_blob |= encode_layout(layout) << (index * 3);
				break;
			default:
				fmt::throw_exception("Unsupported image layout 0x%x", static_cast<u32>(layout));
			}
		}

		inline void set_input_attachment(u32 index)
		{
			input_attachments_mask |= (1ull << index);
		}

		inline void set_format(VkFormat format)
		{
			switch (format)
			{
			case VK_FORMAT_D16_UNORM:
			case VK_FORMAT_D32_SFLOAT:
			case VK_FORMAT_D24_UNORM_S8_UINT:
			case VK_FORMAT_D32_SFLOAT_S8_UINT:
				depth_format = static_cast<u64>(format);
				break;
			default:
				color_format = static_cast<u64>(format);
				break;
			}
		}

		// Decoders
		inline VkSampleCountFlagBits get_sample_count() const
		{
			return static_cast<VkSampleCountFlagBits>(sample_count);
		}

		inline VkFormat get_color_format() const
		{
			return static_cast<VkFormat>(color_format);
		}

		inline VkFormat get_depth_format() const
		{
			return static_cast<VkFormat>(depth_format);
		}

		std::vector<VkAttachmentReference> get_input_attachments() const
		{
			if (input_attachments_mask == 0) [[likely]]
			{
				return {};
			}

			std::vector<VkAttachmentReference> result;
			for (u32 i = 0; i < 5; ++i)
			{
				if (input_attachments_mask & (1ull << i))
				{
					const auto layout = decode_layout((layout_blob >> (i * 3)) & 0x7);
					result.push_back({i, layout});
				}
			}

			return result;
		}

		std::vector<VkImageLayout> get_image_layouts() const
		{
			std::vector<VkImageLayout> result;

			for (u32 i = 0, layout_offset = 0; i < 5; ++i, layout_offset += 3)
			{
				if (const auto layout_encoding = (layout_blob >> layout_offset) & 0x7)
				{
					result.push_back(decode_layout(layout_encoding));
				}
				else
				{
					break;
				}
			}

			return result;
		}
	};
u64 get_renderpass_key(const std::vector<vk::image*>& images, u64 previous_key, const std::vector<u8>& input_attachment_ids)
{
		// Partial update; assumes compatible renderpass keys
		renderpass_key_blob key(previous_key);
		key.layout_blob = 0;
		key.input_attachments_mask = 0;

		for (u32 i = 0; i < ::size32(images); ++i)
		{
			key.set_layout(i, images[i]->current_layout);
		}

		for (const auto& ref_id : input_attachment_ids)
		{
			key.set_input_attachment(ref_id);
		}

		return key.encoded;
	}void validate_image_layout_for_read_access(
		vk::command_buffer& cmd,
		vk::image_view* view,
		VkPipelineStageFlags dst_stage,
		const rsx::sampled_image_descriptor_base* sampler_state)
{
		switch (auto raw = view->image(); +raw->current_layout)
		{
		default:
			//case VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL:
			break;
		case VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL:
			//ensure(sampler_state->upload_context == rsx::texture_upload_context::blit_engine_dst);
			raw->change_layout(cmd, VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL);
			break;
		case VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL:
			ensure(sampler_state->upload_context == rsx::texture_upload_context::blit_engine_src);
			raw->change_layout(cmd, VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL);
			break;
		case VK_IMAGE_LAYOUT_GENERAL:
		case VK_IMAGE_LAYOUT_ATTACHMENT_FEEDBACK_LOOP_OPTIMAL_EXT:
			ensure(sampler_state->upload_context == rsx::texture_upload_context::framebuffer_storage);
			if (sampler_state->is_cyclic_reference) [[ unlikely ]]
			{
				// Nothing to do
				break;
			}

			// This was used in a cyclic ref before, but is missing a barrier
			// No need for a full stall, use a custom barrier instead
			VkPipelineStageFlags src_stage;
			VkAccessFlags src_access, dst_access;
			if (raw->aspect() == VK_IMAGE_ASPECT_COLOR_BIT)
			{
				src_stage = VK_PIPELINE_STAGE_COLOR_ATTACHMENT_OUTPUT_BIT;
				src_access = VK_ACCESS_COLOR_ATTACHMENT_WRITE_BIT;
				dst_access = VK_ACCESS_SHADER_READ_BIT | VK_ACCESS_COLOR_ATTACHMENT_READ_BIT;
				dst_stage |= VK_PIPELINE_STAGE_COLOR_ATTACHMENT_OUTPUT_BIT;
			}
			else
			{
				src_stage = VK_PIPELINE_STAGE_LATE_FRAGMENT_TESTS_BIT;
				src_access = VK_ACCESS_DEPTH_STENCIL_ATTACHMENT_WRITE_BIT;
				dst_access = VK_ACCESS_SHADER_READ_BIT | VK_ACCESS_DEPTH_STENCIL_ATTACHMENT_READ_BIT;
				dst_stage |= VK_PIPELINE_STAGE_EARLY_FRAGMENT_TESTS_BIT | VK_PIPELINE_STAGE_LATE_FRAGMENT_TESTS_BIT;
			}

			vk::insert_image_memory_barrier(
				cmd,
				raw->value,
				raw->current_layout, VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL,
				src_stage, dst_stage,
				src_access, dst_access,
				{ raw->aspect(), 0, 1, 0, 1 });

			raw->current_layout = VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL;
			break;
		case VK_IMAGE_LAYOUT_DEPTH_STENCIL_ATTACHMENT_OPTIMAL:
		case VK_IMAGE_LAYOUT_COLOR_ATTACHMENT_OPTIMAL:
			ensure(sampler_state->upload_context == rsx::texture_upload_context::framebuffer_storage);
			if (!sampler_state->is_cyclic_reference) [[ likely ]]
			{
				// Standard pre-read barrier.
				raw->change_layout(cmd, VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL);
				break;
			}

			// Normally this shouldn't happen. But that is only guaranteed if the attachment state never changes outside of RTT rebind interrupts.
			// If the shader changes between binds to "disable" the cyclic nature, we could end up here.
			// Draw 1 (cyllic) -> texture_barrier -> Draw 2 (no textures) -> attachment_optimal -> Draw 3 (cylic again, no new data) -> incorrect layout.
			vk::as_rtt(raw)->texture_barrier(cmd);
			break;
		}
	}
struct active_renderpass_info_t { VkRenderPass pass=0; VkFramebuffer fbo=0; };
std::map<int,active_renderpass_info_t> g_current_renderpass;
std::map<u64,VkRenderPass> cache; std::map<VkRenderPass,std::vector<VkImageLayout>> pass_layouts;
std::vector<image*> bound; int begins=0, closes=0, creations=0, mismatches=0;
VkRenderPass get_renderpass(int,u64 key) {
    if (cache.contains(key)) return cache.at(key);
    const auto pass=static_cast<VkRenderPass>(++creations); cache[key]=pass;
    pass_layouts[pass]=renderpass_key_blob(key).get_image_layouts(); return pass;
}
void end_renderpass(const command_buffer& cmd) { if(g_current_renderpass[cmd].pass) ++closes; g_current_renderpass[cmd]={}; }
void vkCmdBeginRenderPass(const command_buffer&,const VkRenderPassBeginInfo* info,int) {
    ++begins; const auto& layouts=pass_layouts.at(info->renderPass);
    if(layouts.size()!=bound.size()) { ++mismatches; return; }
    for(usz i=0;i<bound.size();++i) if(bound[i]->current_layout!=layouts[i]) ++mismatches;
}
void begin_renderpass(const vk::command_buffer& cmd, VkRenderPass pass, VkFramebuffer target, const coordu& framebuffer_region)
{
		auto& renderpass_info = g_current_renderpass[cmd];
		if (renderpass_info.pass == pass && renderpass_info.fbo == target)
		{
			return;
		}
		else if (renderpass_info.pass != VK_NULL_HANDLE)
		{
			end_renderpass(cmd);
		}

		VkRenderPassBeginInfo rp_begin = {};
		rp_begin.sType = VK_STRUCTURE_TYPE_RENDER_PASS_BEGIN_INFO;
		rp_begin.renderPass = pass;
		rp_begin.framebuffer = target;
		rp_begin.renderArea.offset.x = static_cast<s32>(framebuffer_region.x);
		rp_begin.renderArea.offset.y = static_cast<s32>(framebuffer_region.y);
		rp_begin.renderArea.extent.width = framebuffer_region.width;
		rp_begin.renderArea.extent.height = framebuffer_region.height;

		vkCmdBeginRenderPass(cmd, &rp_begin, VK_SUBPASS_CONTENTS_INLINE);
		renderpass_info = { pass, target };
	}}

struct descriptor { vk::image_view& view; int sampler; };
struct program { VkImageLayout layout=-1; void bind_uniform(descriptor value,int,int) { layout=value.view.image()->current_layout; } };
struct framebuffer { VkFramebuffer value=1; u32 width() const {return 1280;} u32 height() const {return 720;} };
struct VKGSRender {
    vk::command_buffer commands; vk::command_buffer* m_current_command_buffer=&commands;
    int device=1; int* m_device=&device; VkRenderPass m_cached_renderpass=0; u64 m_current_renderpass_key=0;
    framebuffer fbo; framebuffer* m_draw_fbo=&fbo;
    std::vector<vk::image*> m_fbo_images; std::vector<u8> m_draw_buffers;
    struct {u32 ctrl=0;} current_fragment_program;
    struct {std::pair<int,vk::image*> m_bound_depth_stencil;} m_rtts;
    struct {int frag_depth_input_location=0;} table; decltype(table)* m_fs_binding_table=&table;
    program shader; program* m_program=&shader;
    void begin_render_pass(); VkRenderPass get_render_pass(); void invalidate_render_pass(); void bind_depth_compare();
};
void VKGSRender::begin_render_pass()
{
	// Resource binding can change attachment layouts after load_texture_env().
	// Refresh the layout-only key at the point of use, including resumed passes.
	invalidate_render_pass();

	vk::begin_renderpass(
		*m_current_command_buffer,
		get_render_pass(),
		m_draw_fbo->value,
		{ positionu{0u, 0u}, sizeu{m_draw_fbo->width(), m_draw_fbo->height()} });
}
VkRenderPass VKGSRender::get_render_pass()
{
	if (!m_cached_renderpass)
	{
		m_cached_renderpass = vk::get_renderpass(*m_device, m_current_renderpass_key);
	}

	return m_cached_renderpass;
}
void VKGSRender::invalidate_render_pass()
{
	// Regenerate renderpass key for the next draw call
	std::vector<u8> input_attachments{};
	if (current_fragment_program.ctrl & RSX_SHADER_CONTROL_PROGRAMMABLE_BLENDING)
	{
		input_attachments.resize(m_draw_buffers.size());
		std::iota(input_attachments.begin(), input_attachments.end(), 0);
	}

	if (const auto key = vk::get_renderpass_key(m_fbo_images, m_current_renderpass_key, input_attachments);
		key != m_current_renderpass_key)
	{
		m_current_renderpass_key = key;
		m_cached_renderpass = VK_NULL_HANDLE;
	}
}
void VKGSRender::bind_depth_compare() {
if (current_fragment_program.ctrl & RSX_SHADER_CONTROL_EMULATE_DEPTH_COMPARE)
{
		auto ds = ensure(m_rtts.m_bound_depth_stencil.second);
		// The write/fall-out barrier runs after load_texture_env() and may have
		// restored attachment-only layout. Sampling requires feedback layout again.
		ds->texture_barrier(*m_current_command_buffer);
		auto view = ds->get_view(rsx::default_remap_vector, VK_IMAGE_ASPECT_DEPTH_BIT);
		m_program->bind_uniform({ *view, vk::null_sampler() }, vk::glsl::binding_set_index_fragment, m_fs_binding_table->frag_depth_input_location);
	}
}

int checks=0, failures=0;
void check(const char* name,bool value) { ++checks; if(!value) { ++failures; std::cerr<<"FAIL "<<name<<"\n"; } }
u64 initial_key(const std::vector<vk::image*>& images) {
    vk::renderpass_key_blob key(0); key.color_format=42; key.depth_format=13; key.sample_count=1;
    return vk::get_renderpass_key(images,key.encoded,{});
}
int main() {
    for(bool feedback: {true,false}) {
        vk::image depth; depth.feedback=feedback; vk::image_view view{&depth}; depth.view=&view;
        VKGSRender render; render.m_fbo_images={&depth}; render.m_rtts.m_bound_depth_stencil={0,&depth};
        render.m_current_renderpass_key=initial_key(render.m_fbo_images); vk::bound=render.m_fbo_images;
        const int errors=vk::mismatches;
        render.begin_render_pass(); const auto ordinary=render.m_cached_renderpass;
        check("initial attachment layout matches",vk::mismatches==errors);
        rsx::sampled_image_descriptor_base sampled;
        vk::validate_image_layout_for_read_access(render.commands,&view,VK_PIPELINE_STAGE_FRAGMENT_SHADER_BIT,&sampled);
        const auto loop=depth.current_layout;
        render.begin_render_pass();
        check("late cyclic binding refreshes pass layout",vk::mismatches==errors);
        check("late cyclic layout selects a distinct cached pass",render.m_cached_renderpass!=ordinary);
        const int begins=vk::begins, creations=vk::creations;
        render.begin_render_pass();
        check("unchanged pass remains open without a new begin",vk::begins==begins);
        check("unchanged key reuses cache",vk::creations==creations);
        depth.change_layout(render.commands,VK_IMAGE_LAYOUT_DEPTH_STENCIL_ATTACHMENT_OPTIMAL);
        vk::end_renderpass(render.commands); render.begin_render_pass();
        check("reverse feedback-to-attachment transition refreshes key",vk::mismatches==errors);
        check("return to ordinary layout reuses ordinary pass",render.m_cached_renderpass==ordinary);
        render.current_fragment_program.ctrl=RSX_SHADER_CONTROL_EMULATE_DEPTH_COMPARE;
        render.bind_depth_compare();
        check("depth compare restores a sampleable layout after fall-out",depth.current_layout==loop);
        check("depth descriptor is written with feedback or general layout",render.shader.layout==loop);
        vk::end_renderpass(render.commands); render.begin_render_pass();
        check("depth descriptor and active pass layouts agree",vk::pass_layouts.at(render.m_cached_renderpass).back()==render.shader.layout);
        check("no stale initial layout across cyclic sequence",vk::mismatches==errors);
        render.current_fragment_program.ctrl=0; render.shader.layout=-1;
        const int barriers=depth.barriers; render.bind_depth_compare();
        check("disabled depth compare leaves binding and barriers untouched",render.shader.layout==-1 && depth.barriers==barriers);
        sampled.is_cyclic_reference=false;
        vk::validate_image_layout_for_read_access(render.commands,&view,VK_PIPELINE_STAGE_VERTEX_SHADER_BIT,&sampled);
        check("noncyclic reads transition to shader-read layout",depth.current_layout==VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL);
        vk::end_renderpass(render.commands);
    }
    vk::image colors[4],depth;
    VKGSRender render; render.m_draw_buffers={0,1,2,3};
    for(auto& color:colors) {color.aspects=VK_IMAGE_ASPECT_COLOR_BIT; color.current_layout=VK_IMAGE_LAYOUT_COLOR_ATTACHMENT_OPTIMAL; render.m_fbo_images.push_back(&color);}
    render.m_fbo_images.push_back(&depth); render.m_current_renderpass_key=initial_key(render.m_fbo_images);
    vk::bound=render.m_fbo_images; const int errors=vk::mismatches;
    const auto formats=render.m_current_renderpass_key & ((1ull<<22)-1);
    render.current_fragment_program.ctrl=RSX_SHADER_CONTROL_PROGRAMMABLE_BLENDING;
    colors[2].current_layout=VK_IMAGE_LAYOUT_GENERAL; depth.current_layout=VK_IMAGE_LAYOUT_ATTACHMENT_FEEDBACK_LOOP_OPTIMAL_EXT;
    render.begin_render_pass();
    check("all five attachment layouts refreshed",vk::mismatches==errors);
    check("layout refresh preserves format and sample fields",(render.m_current_renderpass_key & ((1ull<<22)-1))==formats);
    check("programmable blending retains all input attachments",vk::renderpass_key_blob(render.m_current_renderpass_key).input_attachments_mask==15);
    render.current_fragment_program.ctrl=0; vk::end_renderpass(render.commands); render.begin_render_pass();
    check("removed input attachments do not leak from cached key",vk::renderpass_key_blob(render.m_current_renderpass_key).input_attachments_mask==0);
    check("input-mask refresh leaves attachment layouts valid",vk::mismatches==errors);
    std::cout<<checks<<" attachment layout checks, "<<failures<<" failed\n"; return failures?1:0;
}
