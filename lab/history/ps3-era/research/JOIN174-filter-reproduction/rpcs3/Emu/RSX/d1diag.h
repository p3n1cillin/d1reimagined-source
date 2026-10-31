#pragma once
// Destiny (BLES01857) diagnostics: log RSX activity touching the main-memory G-buffer copies
// the SPU lighting reads (0x44000000-0x46000000). Throttled.
#include "util/types.hpp"
#include "Common/descriptor_lifetime_ring.h"
#include <atomic>
#include <bit>
#include <mutex>
#include <vector>
#include <string>
#include "Utilities/StrFmt.h"
#include "Utilities/File.h"
#include "Emu/Cell/timers.hpp"
#include "util/logs.hpp"

LOG_CHANNEL(d1_log, "D1");
namespace d1diag
{
	// Marker files, the PPU watch list and dumps live in "d1/" next to rpcs3.exe, so the whole
	// test setup can be moved or copied as one folder.
	inline const std::string& ctl_dir()
	{
		static const std::string dir = fs::get_executable_dir() + "d1/";
		return dir;
	}

	// Opt-in descriptor lifetime trace. One process-wide budget, including toggles;
	// each successful call reserves one record. Runtime handles are private evidence.
	inline bool descriptor_logging()
	{
		static std::atomic<u64> last_check{0};
		static std::atomic<bool> on{false};
		static std::atomic<u64> records{0};
		static std::mutex poll_mutex;
		const u64 now = get_system_time();
		if (now - last_check.load() > 500'000)
		{
			std::lock_guard lock(poll_mutex);
			if (now > last_check.load() && now - last_check.load() > 500'000)
			{
				on = fs::is_file(ctl_dir() + "d1descriptor");
				last_check = now;
			}
		}
		if (!on.load()) return false;
		constexpr u64 limit = 262'144;
		const u64 record = records.fetch_add(1);
		if (record == limit) d1_log.notice("D1DESC cap=%llu", limit);
		return record < limit;
	}

	// Called inside the queue-submit guard to identify physical submission order.
	inline u64 next_descriptor_submit_order()
	{
		static std::atomic<u64> order{0};
		return order.fetch_add(1) + 1;
	}

	inline bool descriptor_ring_enabled()
	{
		static std::atomic<u64> last_check{0};
		static std::atomic<bool> enabled{false};
		static std::mutex poll_mutex;
		const u64 now = get_system_time();
		if (now - last_check.load() > 500'000)
		{
			std::lock_guard lock(poll_mutex);
			if (now > last_check.load() && now - last_check.load() > 500'000)
			{
				enabled = fs::is_file(ctl_dir() + "d1descriptorring");
				last_check = now;
			}
		}
		return enabled.load();
	}

	inline auto& descriptor_ring()
	{
		static descriptor_lifetime_ring<65'536> ring;
		return ring;
	}

	inline void descriptor_record(descriptor_event_kind kind, std::array<u64, 6> fields)
	{
		if (descriptor_ring_enabled()) descriptor_ring().push(get_system_time(), kind,
			{fields[0], fields[1], fields[2], fields[3], fields[4], fields[5]});
	}

	inline void dump_descriptor_ring()
	{
		if (!descriptor_ring_enabled()) return;
		try
		{
			const auto snapshot = descriptor_ring().freeze();
			if (!snapshot) return;
			d1_log.notice("D1RING begin retained=%llu last=%llu overwritten=%llu reason=validation-descriptor-reset",
				static_cast<u64>(snapshot->records.size()), snapshot->last_serial, snapshot->overwritten);
			for (const auto& record : snapshot->records)
			{
				d1_log.notice("D1RING serial=%llu time_us=%llu kind=%llu a=%016llx b=%016llx c=%016llx d=%016llx e=%016llx f=%016llx",
					record.serial, record.time_us, static_cast<u64>(record.kind),
					record.fields[0], record.fields[1], record.fields[2], record.fields[3], record.fields[4], record.fields[5]);
			}
			d1_log.notice("D1RING end last=%llu", snapshot->last_serial);
		}
		catch (...)
		{
			d1_log.error("D1RING dump failed; original validation error remains authoritative");
		}
	}

	// Timeline tracing, active while the marker file d1/d1trace.on exists (checked every
	// 200 ms), limited to 400000 lines per activation.
	inline bool tracing()
	{
		static std::atomic<u64> last_check{0};
		static std::atomic<bool> on{false};
		static std::atomic<u32> lines{0};
		const u64 now = get_system_time();
		if (now - last_check.load() > 200'000)
		{
			last_check = now;
			const bool was = on.load();
			const bool is = fs::is_file(ctl_dir() + "d1trace.on");
			if (is && !was) lines = 0;
			on = is;
		}
		return on.load() && lines.fetch_add(1) < 400000;
	}
	inline std::atomic<u32> acquire_addrs[16]{};
	inline void note_acquire(u32 addr)
	{
		for (auto& a : acquire_addrs)
		{
			u32 cur = a.load();
			if (cur == addr) return;
			if (cur == 0 && a.compare_exchange_strong(cur, addr)) return;
		}
	}
	inline bool is_acquire_addr(u32 eal, u32 size)
	{
		for (auto& a : acquire_addrs)
		{
			const u32 v = a.load();
			if (v && eal <= v && v < eal + size) return true;
		}
		return false;
	}
	// Light buffer 0xCEB60000 (1024x624 RGBA16F, pitch 8192) and the HUD targets aliasing it.
	inline bool hdr(u32 addr) { return addr >= 0xceb60000u && addr < 0xcf040000u; }
	// Object counters for the D1MEM log line (VKPresent.cpp).
	inline std::atomic<u64> g_programs_created{0}, g_programs_destroyed{0}, g_descriptor_set_first_inits{0}, g_tables_released{0};

	// New fragment program logging (ProgramStateCache.h), while d1/d1fp.on exists, 400 lines
	// per activation.
	inline bool fp_logging()
	{
		static std::atomic<u64> last_check{0};
		static std::atomic<bool> on{false};
		static std::atomic<u32> lines{0};
		const u64 now = get_system_time();
		if (now - last_check.load() > 500'000)
		{
			last_check = now;
			const bool was = on.load();
			const bool is = fs::is_file(ctl_dir() + "d1fp.on");
			if (is && !was) lines = 0;
			on = is;
		}
		return on.load() && lines.fetch_add(1) < 400;
	}


	// Disabled unless explicitly armed for a controlled depth-only comparison.
	inline bool depth_only_enabled()
	{
		static u64 last_check = 0;
		static bool enabled = false;
		const u64 now = get_system_time();
		if (now - last_check > 500'000)
		{
			last_check = now;
			enabled = fs::is_file(ctl_dir() + "d1depthfp.on");
		}
		return enabled;
	}

	inline bool hit(u32 addr, u32 len)
	{
		return addr < 0x46000000u && addr + len > 0x44000000u;
	}
	inline bool sampled()
	{
		static std::atomic<u32> n{0};
		return (n.fetch_add(1) & 255) == 0;
	}
	inline bool sampled16()
	{
		static std::atomic<u32> n{0};
		return (n.fetch_add(1) & 15) == 0;
	}
	inline bool budget()
	{
		static std::atomic<u32> n{0};
		return n.fetch_add(1) < 40000;
	}

	// One-frame surface dump: creating d1/d1dump.on arms it at the next flip (the marker is
	// consumed); every render/depth target is written to d1/dump/<id>/ when the RSX
	// switches away from it, until the first target switch after the following flip. RSX thread only.
	struct dump_state
	{
		bool active = false;
		bool stop_next = false;
		u32 seq = 0;
		std::string dir;
	};
	inline dump_state g_dump;
	inline void dump_on_flip()
	{
		if (g_dump.active)
		{
			g_dump.stop_next = true;
			return;
		}
		if (fs::is_file(ctl_dir() + "d1dump.on"))
		{
			fs::remove_file(ctl_dir() + "d1dump.on");
			g_dump.active = true;
			g_dump.stop_next = false;
			g_dump.seq = 0;
			g_dump.dir = fmt::format("%sdump/%llu/", ctl_dir(), get_system_time());
			fs::create_path(g_dump.dir);
		}
	}
	inline bool dumping() { return g_dump.active; }
	inline void dump_bytes(const void* data, u32 size, u32 addr, u32 pitch, u32 width, u32 height, int format, char kind)
	{
		fs::file(fmt::format("%s%04u_%c_%08x_p%u_%ux%u_f%d.bin", g_dump.dir, g_dump.seq++, kind, addr, pitch, width, height, format),
			fs::rewrite).write(data, size);
	}
	inline void dump_done()
	{
		if (g_dump.stop_next) g_dump.active = false;
	}

	// PPU call tracing (LLVM decoder, where GDB breakpoints are ignored). Every compiled function
	// below 0x2000000 checks g_ppu_mask[addr >> 2] on entry and calls ppu_d1hit when set. The
	// watch list comes from d1/d1ppu.txt, one watch per line: "ADDR [MAXHITS] [u] [rN[+OFF]:LEN ...]"
	// (hex address, default 50 hits, memory dumps at register + offset); re-read when it changes.
	// Fixed host address (reserved and committed in PPUThread.cpp before any PPU code runs), so the
	// compiled PPU code can embed it as a 64-bit constant and cached objects stay valid across runs.
	constexpr u64 ppu_mask_addr = 0x6d0000000000;
	inline u8* const g_ppu_mask = reinterpret_cast<u8*>(ppu_mask_addr);
	struct ppu_watch
	{
		u32 addr;
		u32 left;
		std::vector<std::tuple<u32, u32, u32>> dumps; // register, offset, length
		bool unique = false; // "u": log each distinct dump content once
		std::vector<u64> seen;
	};
	inline std::mutex g_ppu_mutex;
	inline std::vector<ppu_watch> g_ppu_watches;
	inline void ppu_poll()
	{
		static u64 last_check = 0;
		static std::string last_text;
		const u64 now = get_system_time();
		if (now - last_check < 1'000'000) return;
		last_check = now;
		std::string text;
		if (fs::file f{ctl_dir() + "d1ppu.txt"}) text = f.to_string();
		if (text == last_text) return;
		last_text = text;
		std::lock_guard lock(g_ppu_mutex);
		for (const auto& w : g_ppu_watches) g_ppu_mask[w.addr >> 2] = 0;
		g_ppu_watches.clear();
		// A line "coverage" arms every function for one logged first call (D1C lines); it is
		// armed once per appearance of the line. Every reload is marked in the log.
		static bool coverage = false;
		const bool want_coverage = text.find("coverage") != umax;
		if (want_coverage && !coverage)
		{
			for (u32 i = 0; i < 0x800000; i++) if (!g_ppu_mask[i]) g_ppu_mask[i] = 2;
		}
		else if (!want_coverage && coverage)
		{
			for (u32 i = 0; i < 0x800000; i++) if (g_ppu_mask[i] == 2) g_ppu_mask[i] = 0;
		}
		coverage = want_coverage;
		d1_log.warning("D1P watch list reloaded at %llu (coverage %d)", now, coverage);
		usz pos = 0;
		while (pos < text.size())
		{
			usz end = text.find('\n', pos);
			if (end == umax) end = text.size();
			const std::string line = text.substr(pos, end - pos);
			pos = end + 1;
			ppu_watch w{};
			w.left = 50;
			bool first = true;
			try {
			usz i = 0;
			while (i < line.size())
			{
				while (i < line.size() && (line[i] == ' ' || line[i] == '\t' || line[i] == '\r')) i++;
				usz j = i;
				while (j < line.size() && line[j] != ' ' && line[j] != '\t' && line[j] != '\r') j++;
				if (j == i) break;
				const std::string tok = line.substr(i, j - i);
				i = j;
				if (tok[0] == '#') break;
				if (first)
				{
					w.addr = static_cast<u32>(std::stoul(tok, nullptr, 16));
					first = false;
				}
				else if (tok == "u")
				{
					w.unique = true;
				}
				else if (tok[0] == 'r' && tok.find(':') != umax)
				{
					const usz colon = tok.find(':'), plus = tok.find('+');
					const u32 off = plus < colon ? static_cast<u32>(std::stoul(tok.substr(plus + 1, colon - plus - 1), nullptr, 0)) : 0;
					w.dumps.emplace_back(static_cast<u32>(std::stoul(tok.substr(1))), off, static_cast<u32>(std::stoul(tok.substr(colon + 1), nullptr, 0)));
				}
				else
				{
					w.left = static_cast<u32>(std::stoul(tok, nullptr, 0));
				}
			}
			} catch (...) { continue; }
			if (!first && w.addr < 0x2000000 && !(w.addr & 3))
			{
				g_ppu_watches.push_back(w);
				g_ppu_mask[w.addr >> 2] = 1;
			}
		}
	}
}
