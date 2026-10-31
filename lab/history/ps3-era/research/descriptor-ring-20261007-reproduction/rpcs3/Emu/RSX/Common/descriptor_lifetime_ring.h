#pragma once

#include <array>
#include <atomic>
#include <cstdint>
#include <mutex>
#include <optional>
#include <vector>

namespace d1diag
{
	enum class descriptor_event_kind : std::uint64_t
	{
		tag = 1, submit, completed_poke, completed_wait, retire, dispose, collect, reset
	};

	struct descriptor_event
	{
		std::uint64_t serial = 0;
		std::uint64_t time_us = 0;
		descriptor_event_kind kind{};
		std::array<std::uint64_t, 6> fields{};
	};

	// Writers serialize only a fixed-size copy. Freeze takes a consistent suffix
	// exactly once; later writers cannot overwrite records being inspected.
	template<std::size_t Capacity>
	class descriptor_lifetime_ring
	{
		static_assert(Capacity > 0);
		std::array<descriptor_event, Capacity> m_records{};
		std::mutex m_mutex;
		std::atomic<bool> m_frozen{false};
		std::uint64_t m_serial = 0;

	public:
		struct snapshot
		{
			std::uint64_t last_serial = 0;
			std::uint64_t overwritten = 0;
			std::vector<descriptor_event> records;
		};

		bool push(std::uint64_t time_us, descriptor_event_kind kind,
			std::array<std::uint64_t, 6> fields)
		{
			if (m_frozen.load(std::memory_order_acquire)) return false;
			std::lock_guard lock(m_mutex);
			if (m_frozen.load(std::memory_order_relaxed)) return false;
			const auto serial = ++m_serial;
			m_records[(serial - 1) % Capacity] = {serial, time_us, kind, fields};
			return true;
		}

		std::optional<snapshot> freeze()
		{
			std::lock_guard lock(m_mutex);
			if (m_frozen.exchange(true, std::memory_order_acq_rel)) return std::nullopt;
			snapshot result;
			result.last_serial = m_serial;
			const auto count = m_serial < Capacity ? m_serial : Capacity;
			result.overwritten = m_serial - count;
			result.records.reserve(static_cast<std::size_t>(count));
			for (auto serial = m_serial - count + 1; serial <= m_serial; ++serial)
			{
				result.records.push_back(m_records[(serial - 1) % Capacity]);
			}
			return result;
		}
	};
}
