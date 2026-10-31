
#include <algorithm>
#include <cstdint>
#include <iostream>
#include <memory>
#include <set>
#include <tuple>
#include <utility>
#include <vector>
using u32 = std::uint32_t;
using VkFormat = int;
constexpr int VK_IMAGE_TYPE_2D = 1, VK_IMAGE_USAGE_SAMPLED_BIT = 2;
constexpr int VK_IMAGE_USAGE_TRANSFER_SRC_BIT = 4, VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL = 5;
constexpr int CELL_GCM_LOCATION_LOCAL = 0, RSX_FORMAT_CLASS_COLOR = 0;
struct areai { int x1, y1, x2, y2; };
namespace utils {
struct address_range32 { static int start_length(u32, u32) { return 0; } };
}
namespace rsx {
enum class surface_metrics { samples };
enum class surface_access { transfer_read };
enum class invalidation_cause { read };
struct avconf { int format = 1; };
namespace texture_upload_context { constexpr u32 blit_engine_dst = 1, framebuffer_storage = 2; }
int get_format_block_size_in_bytes(int) { return 4; }
template<bool> std::pair<u32, u32> apply_resolution_scale(int, u32 w, u32 h) { return {w, h}; }
}
int RSX_display_format_to_vk_format(int format) { return format; }
namespace vk {
std::set<const void*> live;
int recycled = 0, destroyed = 0, direct_copies = 0, typeless_copies = 0;
struct command_buffer { u32 flags{}; static constexpr u32 cb_has_dma_transfer = 1; };
struct viewable_image {
    int fmt; u32 w = 1280, h = 720;
    explicit viewable_image(int format = 1): fmt(format) { live.insert(this); }
    virtual ~viewable_image() { live.erase(this); ++destroyed; }
    int format() const { return fmt; }
    void change_layout(command_buffer&, int) {}
};
struct render_target : viewable_image {
    using viewable_image::viewable_image;
    template<rsx::surface_metrics> u32 get_surface_width() { return w; }
    template<rsx::surface_metrics> u32 get_surface_height() { return h; }
    viewable_image* get_surface(rsx::surface_access) { return this; }
};
render_target* as_rtt(render_target* r) { return r; }
struct present_surface_info { u32 address=0xc0001000, format=1, width=1280, height=720, pitch=5120; };
struct retirement { std::unique_ptr<viewable_image> data; bool reuse; };
struct fake_gc {
    std::vector<retirement> pending;
    std::vector<std::unique_ptr<viewable_image>> pool;
    void dispose(std::unique_ptr<viewable_image>& image) { pending.push_back({std::move(image), false}); }
    void recycle(std::unique_ptr<viewable_image>& image) { pending.push_back({std::move(image), true}); }
    void complete_submission() {
        for (auto& r : pending) {
            if (r.reuse) { ++recycled; pool.push_back(std::move(r.data)); }
            else r.data.reset();
        }
        pending.clear();
    }
} gc;
fake_gc* get_resource_manager() { return &gc; }
bool formats_are_bitcast_compatible(viewable_image*, viewable_image*) { return typeless_copies == -1; }
void copy_image(command_buffer&, viewable_image*, viewable_image*, const areai&, const areai&) { ++direct_copies; }
void copy_image_typeless(command_buffer&, viewable_image*, viewable_image*, const areai&, const areai&) { ++typeless_copies; }
}
struct overlap { vk::render_target* surface; u32 base_address; };
struct rtt_store {
    vk::render_target image; bool available = false; u32 address = 0xc0001000;
    std::vector<overlap> get_merged_texture_memory_region(vk::command_buffer&, u32, u32, u32, u32, int, rsx::surface_access) {
        return available ? std::vector<overlap>{{&image, address}} : std::vector<overlap>{};
    }
};
struct cached_texture {
    vk::viewable_image image;
    u32 get_width() { return image.w; } u32 get_height() { return image.h; }
    vk::viewable_image* get_raw_texture() { return &image; }
    bool is_synchronized() { return true; }
    void copy_texture(vk::command_buffer&, bool) {}
};
struct texture_store {
    cached_texture cached; bool available = false, fail_upload = false, fail_conversion = false;
    template<bool> cached_texture* find_texture_from_dimensions(u32, u32) { return available ? &cached : nullptr; }
    template<bool> std::vector<cached_texture*> find_texture_from_range(int, int, u32) { return {}; }
    void invalidate_range(vk::command_buffer&, int, rsx::invalidation_cause) {}
    auto create_temporary_subresource_storage(int, int format, u32, u32, int, int, int, int, int, int) {
        return fail_conversion ? std::unique_ptr<vk::viewable_image>{} : std::make_unique<vk::viewable_image>(format);
    }
    void dispose_reusable_image(std::unique_ptr<vk::viewable_image>& image) { vk::gc.recycle(image); }
    std::unique_ptr<vk::viewable_image> upload_image_simple(vk::command_buffer&, int format, u32, u32, u32, u32) {
        auto image = fail_upload ? std::unique_ptr<vk::viewable_image>{} : std::make_unique<vk::viewable_image>(format);
        return image;
    }
};
struct VKGSRender {
    struct present_source {
		vk::viewable_image* image = nullptr;
		std::unique_ptr<vk::viewable_image> owned;
		bool reusable = false;
	};
    rtt_store m_rtts; texture_store m_texture_cache;
    vk::command_buffer cmd; vk::command_buffer* m_current_command_buffer = &cmd;
    int resolution_scaling_config = 100;
    VKGSRender::present_source get_present_source(vk::present_surface_info*, const rsx::avconf&);
    void flush_command_queue() { vk::gc.complete_submission(); }
    present_source select(vk::present_surface_info* info, const rsx::avconf& config) { return get_present_source(info, config); }
    void finish(present_source (&present_sources)[2]) { for (auto& source : present_sources) {
		if (!source.owned) continue;
		if (source.reusable)
			m_texture_cache.dispose_reusable_image(source.owned);
		else
			vk::get_resource_manager()->dispose(source.owned);
	} }
};
VKGSRender::present_source VKGSRender::get_present_source(vk::present_surface_info* info, const rsx::avconf& avconfig)
{
	present_source result;
	auto& image_to_flip = result.image;

	// @FIXME: This entire function needs to be rewritten to go through the texture cache's "upload_texture" routine.
	// That method is not a 1:1 replacement due to handling of insets that is done differently here.

	// Check the surface store first
	const auto format_bpp = rsx::get_format_block_size_in_bytes(info->format);
	const auto overlap_info = m_rtts.get_merged_texture_memory_region(*m_current_command_buffer,
		info->address, info->width, info->height, info->pitch, format_bpp, rsx::surface_access::transfer_read);

	if (!overlap_info.empty())
	{
		const auto& section = overlap_info.back();
		auto surface = vk::as_rtt(section.surface);
		bool viable = false;

		if (section.base_address >= info->address)
		{
			const auto surface_width = surface->get_surface_width<rsx::surface_metrics::samples>();
			const auto surface_height = surface->get_surface_height<rsx::surface_metrics::samples>();

			if (section.base_address == info->address)
			{
				// Check for fit or crop
				viable = (surface_width >= info->width && surface_height >= info->height);
			}
			else
			{
				// Check for borders and letterboxing
				const u32 inset_offset = section.base_address - info->address;
				const u32 inset_y = inset_offset / info->pitch;
				const u32 inset_x = (inset_offset % info->pitch) / format_bpp;

				const u32 full_width = surface_width + inset_x + inset_x;
				const u32 full_height = surface_height + inset_y + inset_y;

				viable = (full_width == info->width && full_height == info->height);
			}

			if (viable)
			{
				image_to_flip = section.surface->get_surface(rsx::surface_access::transfer_read);

				std::tie(info->width, info->height) = rsx::apply_resolution_scale<true>(
					resolution_scaling_config,
					std::min(surface_width, info->width),
					std::min(surface_height, info->height));
			}
		}
	}
	else if (auto surface = m_texture_cache.find_texture_from_dimensions<true>(info->address, info->format);
			 surface && surface->get_width() >= info->width && surface->get_height() >= info->height)
	{
		// Hack - this should be the first location to check for output
		// The render might have been done offscreen or in software and a blit used to display
		image_to_flip = dynamic_cast<vk::viewable_image*>(surface->get_raw_texture());
	}

	// The correct output format is determined by the AV configuration set in CellVideoOutConfigure by the game.
	// 99.9% of the time, this will match the backbuffer fbo format used in rendering/compositing the output.
	// But in some cases, let's just say some devs are creative.
	const auto expected_format = RSX_display_format_to_vk_format(avconfig.format);

	if (!image_to_flip) [[ unlikely ]]
	{
		// Read from cell
		const auto range = utils::address_range32::start_length(info->address, info->pitch * info->height);
		const u32  lookup_mask = rsx::texture_upload_context::blit_engine_dst | rsx::texture_upload_context::framebuffer_storage;
		const auto overlap = m_texture_cache.find_texture_from_range<true>(range, 0, lookup_mask);

		for (const auto & section : overlap)
		{
			if (!section->is_synchronized())
			{
				section->copy_texture(*m_current_command_buffer, true);
			}
		}

		if (m_current_command_buffer->flags & vk::command_buffer::cb_has_dma_transfer)
		{
			// Submit for processing to lower hard fault penalty
			flush_command_queue();
		}

		m_texture_cache.invalidate_range(*m_current_command_buffer, range, rsx::invalidation_cause::read);
		result.owned = m_texture_cache.upload_image_simple(*m_current_command_buffer, expected_format, info->address, info->width, info->height, info->pitch);
		image_to_flip = result.owned.get();
	}
	else if (image_to_flip->format() != expected_format)
	{
		// Devs are being creative. Force-cast this to the proper pixel layout.
		auto dst_img = m_texture_cache.create_temporary_subresource_storage(
			RSX_FORMAT_CLASS_COLOR, expected_format, info->width, info->height, 1, 1, 1,
			VK_IMAGE_TYPE_2D, 0, VK_IMAGE_USAGE_SAMPLED_BIT | VK_IMAGE_USAGE_TRANSFER_SRC_BIT);

		if (dst_img)
		{
			const areai src_rect = { 0, 0, static_cast<int>(info->width), static_cast<int>(info->height) };
			const areai dst_rect = src_rect;

			dst_img->change_layout(*m_current_command_buffer, VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL);

			if (vk::formats_are_bitcast_compatible(dst_img.get(), image_to_flip))
			{
				vk::copy_image(*m_current_command_buffer, image_to_flip, dst_img.get(), src_rect, dst_rect);
			}
			else
			{
				vk::copy_image_typeless(*m_current_command_buffer, image_to_flip, dst_img.get(), src_rect, dst_rect);
			}

			image_to_flip = dst_img.get();
			result.owned = std::move(dst_img);
			result.reusable = true;
		}
	}

	return result;
}
int main() {
    int checks = 0, failures = 0;
    auto check = [&](const char* name, bool good) { ++checks; if(!good) { ++failures; std::cout << "FAIL " << name << '\n'; } };
    for(int mode = 0; mode < 6; ++mode) {
        vk::gc.complete_submission(); vk::gc.pool.clear();
        VKGSRender renderer;
        if(mode == 0 || mode == 3 || mode == 4) renderer.m_rtts.available = true;
        if(mode == 1) renderer.m_texture_cache.available = true;
        if(mode == 3 || mode == 4) renderer.m_rtts.image.fmt = 2;
        if(mode == 4) renderer.m_texture_cache.fail_conversion = true;
        if(mode == 5) renderer.m_texture_cache.fail_upload = true;
        vk::present_surface_info info; rsx::avconf config;
        VKGSRender::present_source sources[2];
        sources[0] = renderer.select(&info, config);
        sources[1] = renderer.select(&info, config);
        auto* left = sources[0].image; auto* right = sources[1].image;
        check("both eyes selected or upload rejected", mode == 5 ? !left && !right : left && right);
        const int recycled_before = vk::recycled, destroyed_before = vk::destroyed;
        // Recording submits and waits for the upload/conversion commands. This
        // completion may run the same deferred retirements as the real renderer.
        vk::gc.complete_submission();
        check("media sync leaves left source alive", !left || vk::live.contains(left));
        check("media sync leaves right source alive", !right || vk::live.contains(right));
        check("media sync cannot recycle presentation source", vk::recycled == recycled_before);
        check("media sync cannot destroy presentation source", vk::destroyed == destroyed_before);
        // Record the final presentation use, queue retirement, then complete it.
        renderer.finish(sources);
        const bool owns = mode == 2 || mode == 3;
        check("only owned sources retire at final use", vk::gc.pending.size() == (owns ? 2u : 0u));
        vk::gc.complete_submission();
        if(mode == 2) check("linear images destroyed after final submission", vk::destroyed == destroyed_before + 2);
        if(mode == 3) check("conversion images recycled after final submission", vk::recycled == recycled_before + 2);
        if(mode == 0 || mode == 1 || mode == 4) check("borrowed sources preserved", vk::live.contains(left) && vk::live.contains(right));
    }
    std::cout << checks << " lifetime checks, " << failures << " failed\n";
    return failures ? 1 : 0;
}
