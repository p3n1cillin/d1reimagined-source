
#include <cstdint>
#include <iostream>
#include <memory>
using u8 = std::uint8_t; using u16 = std::uint16_t; using u32 = std::uint32_t;
using VkFlags = u32; using VkFormat = int; using VkImageType = int;
constexpr u32 VK_IMAGE_USAGE_TRANSFER_SRC_BIT = 1, VK_IMAGE_USAGE_TRANSFER_DST_BIT = 2;
constexpr u32 VK_IMAGE_USAGE_SAMPLED_BIT = 4, VK_IMAGE_USAGE_STORAGE_BIT = 8;
constexpr u32 VK_IMAGE_USAGE_COLOR_ATTACHMENT_BIT = 16;
constexpr int VK_MEMORY_PROPERTY_DEVICE_LOCAL_BIT = 1, VK_SAMPLE_COUNT_1_BIT = 1;
constexpr int VK_IMAGE_LAYOUT_UNDEFINED = 0, VK_IMAGE_TILING_OPTIMAL = 0;
constexpr int VK_SHARING_MODE_EXCLUSIVE = 0, VK_IMAGE_CREATE_ALLOW_NULL_RPCS3 = 0;
constexpr int VMM_ALLOCATION_POOL_TEXTURE_CACHE = 0;
namespace rsx { using format_class = int; }
namespace vk {
bool allocation_fails = false;
int renderer = 0, releases = 0;
int* get_current_renderer() { return &renderer; }
struct viewable_image {
    int value = 1; struct { u32 usage; } info;
    explicit viewable_image(u32 usage): info{usage} {}
    viewable_image(int, int, int, VkImageType, VkFormat, u16, u16, u16, u8, u16, int, int, int,
                   u32 usage, int, int, rsx::format_class): value(!allocation_fails), info{usage} {}
    ~viewable_image() { ++releases; }
};
struct texture_cache {
    struct { int device_local = 0; } m_memory_types;
    std::unique_ptr<viewable_image> cached;
    u32 last_lookup = 0;
    std::unique_ptr<viewable_image> find_cached_image(VkFormat, u16, u16, u16, u8, VkImageType, VkFlags, VkFlags usage, int) {
        last_lookup = usage;
        if(cached && (cached->info.usage & usage) == usage) return std::move(cached);
        return {};
    }
    std::unique_ptr<viewable_image> create_temporary_subresource_storage(
        rsx::format_class, VkFormat, u16, u16, u16, u16, u8, VkImageType, VkFlags, VkFlags);
};
std::unique_ptr<vk::viewable_image> texture_cache::create_temporary_subresource_storage(
    rsx::format_class format_class, VkFormat format, u16 width, u16 height, u16 depth, u16 layers,
    u8 mips, VkImageType image_type, VkFlags image_flags, VkFlags usage_flags)
{
		auto image = find_cached_image(format, width, height, depth, mips, image_type, image_flags, usage_flags, VK_SHARING_MODE_EXCLUSIVE);

		if (!image)
		{
			image = std::make_unique<vk::viewable_image>(*vk::get_current_renderer(), m_memory_types.device_local, VK_MEMORY_PROPERTY_DEVICE_LOCAL_BIT,
				image_type,
				format,
				width, height, depth, mips, layers, VK_SAMPLE_COUNT_1_BIT, VK_IMAGE_LAYOUT_UNDEFINED,
				VK_IMAGE_TILING_OPTIMAL, VK_IMAGE_USAGE_TRANSFER_DST_BIT | VK_IMAGE_USAGE_SAMPLED_BIT, image_flags | VK_IMAGE_CREATE_ALLOW_NULL_RPCS3,
				VMM_ALLOCATION_POOL_TEXTURE_CACHE, format_class);

			if (!image->value)
			{
				// OOM, bail
				return nullptr;
			}
		}

		return image;
	}
}
int main() {
    int checks = 0, failures = 0;
    auto check = [&](const char* name, bool ok) { ++checks; if(!ok) { ++failures; std::cout << "FAIL " << name << '\n'; } };
    for(u32 requested : {0u, VK_IMAGE_USAGE_SAMPLED_BIT, VK_IMAGE_USAGE_TRANSFER_SRC_BIT,
                         VK_IMAGE_USAGE_STORAGE_BIT, VK_IMAGE_USAGE_COLOR_ATTACHMENT_BIT}) {
        vk::texture_cache cache;
        const u32 required = requested | VK_IMAGE_USAGE_TRANSFER_DST_BIT | VK_IMAGE_USAGE_SAMPLED_BIT;
        auto create = [&] { return cache.create_temporary_subresource_storage(0, 1, 1280, 720, 1, 1, 1, 1, 0, requested); };
        auto image = create();
        check("new image preserves all required uses", image && image->info.usage == required);
        check("lookup and allocation use the same mask", cache.last_lookup == required);
        cache.cached = std::make_unique<vk::viewable_image>(requested);
        const auto* unsuitable = cache.cached.get();
        image = create();
        check("image lacking mandatory uses is not reused", image.get() != unsuitable);
        cache.cached = std::make_unique<vk::viewable_image>(required);
        const auto* suitable = cache.cached.get();
        image = create();
        check("compatible image is still reused", image.get() == suitable);
    }
    vk::texture_cache cache; vk::allocation_fails = true;
    const int released_before = vk::releases;
    auto image = cache.create_temporary_subresource_storage(0, 1, 1280, 720, 1, 1, 1, 1, 0, VK_IMAGE_USAGE_TRANSFER_SRC_BIT);
    check("allocation failure returns no image and releases failed allocation", !image && vk::releases == released_before + 1);
    std::cout << checks << " image usage checks, " << failures << " failed\n";
    return failures ? 1 : 0;
}
