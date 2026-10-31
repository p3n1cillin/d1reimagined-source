#pragma once

#include <util/types.hpp>

#include <functional>

namespace vk
{
	class disposable_t
	{
		void* ptr;
		std::function<void(void*)> deleter;

		disposable_t(void* ptr_, std::function<void(void*)> deleter_) :
			ptr(ptr_), deleter(deleter_) {}
	public:

		disposable_t() = delete;
		disposable_t(const disposable_t&) = delete;

		disposable_t(disposable_t&& other) noexcept :
			ptr(std::exchange(other.ptr, nullptr)),
			deleter(other.deleter)
		{}

		~disposable_t()
		{
			if (ptr)
			{
				deleter(ptr);
				ptr = nullptr;
			}
		}

		// Read-only identity for bounded lifetime diagnostics; no ownership transfer.
		const void* debug_handle() const { return ptr; }

		template <typename T>
		static disposable_t make(T* raw)
		{
			return disposable_t(raw, [](void* raw)
			{
				delete static_cast<T*>(raw);
			});
		}
	};

	struct garbage_collector
	{
		virtual void dispose(vk::disposable_t& object) = 0;

		virtual void add_exit_callback(std::function<void()> callback) = 0;

		template<typename T>
		void dispose(std::unique_ptr<T>& object)
		{
			auto ptr = vk::disposable_t::make(object.release());
			dispose(ptr);
		}
	};

	garbage_collector* get_gc();
}
