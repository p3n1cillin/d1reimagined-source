#pragma once
#include <charconv>
#include <cstdint>
#include <limits>
#include <stdexcept>
#include <string_view>

namespace d1diag
{
	inline constexpr std::uint32_t ppu_filter_lr = 32; // internal selector; r32 is not a register
	// A register equality, or a big-endian guest-memory equality. A zero width selects
	// the register itself. Filters are read-only and must run before deduplication/budget.
	struct ppu_watch_filter
	{
		std::uint32_t reg = 0, offset = 0, width = 0;
		std::uint64_t value = 0;
	};

	inline bool ppu_filter_number(std::string_view text, std::uint64_t& value, bool allow_hex = true)
	{
		int base = 10;
		if (allow_hex && text.starts_with("0x"))
		{
			text.remove_prefix(2);
			base = 16;
		}
		if (text.empty()) return false;
		const auto result = std::from_chars(text.data(), text.data() + text.size(), value, base);
		return result.ec == std::errc{} && result.ptr == text.data() + text.size();
	}

	// if=r5==0x15, if=lr==0x47a208 or if=m6+1:4==0x15; predicates are ANDed in order.
	inline bool parse_ppu_watch_filter(std::string_view token, ppu_watch_filter& out)
	{
		if (!token.starts_with("if=")) return false;
		token.remove_prefix(3);
		const auto equal = token.find("==");
		if (equal == std::string_view::npos) return false;
		auto location = token.substr(0, equal);
		ppu_watch_filter parsed;
		if (!ppu_filter_number(token.substr(equal + 2), parsed.value) || location.empty()) return false;
		if (location == "lr")
		{
			parsed.reg = ppu_filter_lr;
			out = parsed;
			return true;
		}
		const bool memory = location.front() == 'm';
		if (!memory && location.front() != 'r') return false;
		location.remove_prefix(1);
		const auto plus = location.find('+'), colon = location.find(':');
		std::uint64_t reg = 0, offset = 0, width = 0;
		const auto reg_end = memory ? (plus < colon ? plus : colon) : location.size();
		// r0/r2 travel in the LLVM entry ABI and are not exposed by the existing hook.
		if (!ppu_filter_number(location.substr(0, reg_end), reg, false) || reg > 31 || reg == 0 || reg == 2) return false;
		if (memory)
		{
			if (colon == std::string_view::npos) return false;
			if (plus != std::string_view::npos && (plus >= colon || !ppu_filter_number(location.substr(plus + 1, colon - plus - 1), offset))) return false;
			if (offset > std::numeric_limits<std::uint32_t>::max() || !ppu_filter_number(location.substr(colon + 1), width, false)) return false;
			if (width != 1 && width != 2 && width != 4 && width != 8) return false;
			if (width != 8 && parsed.value >= (std::uint64_t{1} << (width * 8))) return false;
		}
		parsed.reg = static_cast<std::uint32_t>(reg);
		parsed.offset = static_cast<std::uint32_t>(offset);
		parsed.width = static_cast<std::uint32_t>(width);
		out = parsed;
		return true;
	}

	template <typename RegisterRead, typename MemoryRead>
	bool matches_ppu_watch_filter(const ppu_watch_filter& filter, RegisterRead&& reg, MemoryRead&& memory)
	{
		const auto base = reg(filter.reg);
		if (!filter.width) return base == filter.value;
		const std::uint64_t address = static_cast<std::uint32_t>(base) + std::uint64_t{filter.offset};
		if (address + filter.width > (std::uint64_t{1} << 32)) return false;
		std::uint64_t value = 0;
		return memory(static_cast<std::uint32_t>(address), filter.width, value) && value == filter.value;
	}
}
