
#include "descriptor_lifetime_ring.h"
#include <algorithm>
#include <iostream>
#include <memory>
#include <set>
#include <thread>

using namespace d1diag;
static unsigned checks = 0, failures = 0;
void check(bool value, const char* label) {
    ++checks;
    failures += !value;
    std::cout << (value ? "PASS " : "FAIL ") << label << '\n';
}
int main() {
    auto empty = std::make_unique<descriptor_lifetime_ring<4>>();
    auto first = empty->freeze();
    check(first && first->records.empty() && first->last_serial == 0, "empty freeze");
    check(!empty->freeze(), "one-shot freeze");
    check(!empty->push(1, descriptor_event_kind::tag, {}), "frozen writer rejected");

    auto small = std::make_unique<descriptor_lifetime_ring<4>>();
    check(small->push(10, descriptor_event_kind::submit, {1,2,3,4,~0ull,6}), "accept first event");
    for (std::uint64_t i = 2; i <= 10; ++i)
        small->push(i*10, descriptor_event_kind::retire, {i,i+1,i+2,i+3,i+4,i+5});
    auto suffix = small->freeze();
    check(suffix && suffix->records.size() == 4, "fixed capacity");
    check(suffix->last_serial == 10 && suffix->overwritten == 6, "explicit overwrite count");
    check(suffix->records.front().serial == 7 && suffix->records.back().serial == 10, "newest contiguous suffix");
    check(suffix->records[0].time_us == 70 && suffix->records[3].time_us == 100, "timestamps retained");
    check(suffix->records[0].kind == descriptor_event_kind::retire && suffix->records[3].fields[5] == 15, "kind and fields retained");
    check(!small->push(200, descriptor_event_kind::reset, {}) && !small->freeze(), "overflow freeze stays immutable");

    auto bits = std::make_unique<descriptor_lifetime_ring<1>>();
    bits->push(0, descriptor_event_kind::submit, {~0ull,0,1,2,~0ull-3,5});
    const auto binary = bits->freeze();
    check(binary->records[0].fields[0] == ~0ull && binary->records[0].fields[4] == ~0ull-3, "full-width handles and negative result bits");

    auto concurrent = std::make_unique<descriptor_lifetime_ring<8192>>();
    std::vector<std::thread> writers;
    for (unsigned worker = 0; worker < 4; ++worker) writers.emplace_back([&,worker] {
        for (unsigned i = 0; i < 2000; ++i)
            concurrent->push(i, descriptor_event_kind::dispose, {worker,i,worker^i});
    });
    for (auto& writer : writers) writer.join();
    auto many = concurrent->freeze();
    check(many->records.size() == 8000 && many->last_serial == 8000 && many->overwritten == 0, "all concurrent events retained");
    bool ordered = true, intact = true;
    std::set<std::pair<std::uint64_t,std::uint64_t>> unique;
    for (std::size_t i = 0; i < many->records.size(); ++i) {
        const auto& r = many->records[i];
        ordered &= r.serial == i+1;
        intact &= r.kind == descriptor_event_kind::dispose && r.fields[0] < 4 && r.fields[1] < 2000 &&
                  r.fields[2] == (r.fields[0]^r.fields[1]) && r.time_us == r.fields[1];
        unique.emplace(r.fields[0],r.fields[1]);
    }
    check(ordered, "concurrent serialization has no holes");
    check(intact, "concurrent records never tear");
    check(unique.size() == 8000, "concurrent identities occur once");

    auto raced = std::make_unique<descriptor_lifetime_ring<64>>();
    std::atomic<bool> ready{false};
    std::atomic<unsigned> accepted{0};
    std::thread producer([&] {
        for (unsigned i = 0; i < 100000; ++i) {
            if (!raced->push(i, descriptor_event_kind::collect, {i,i^0x55u})) break;
            ++accepted;
            ready = true;
        }
    });
    while (!ready.load()) std::this_thread::yield();
    const auto frozen = raced->freeze();
    producer.join();
    check(frozen && frozen->last_serial == accepted.load(), "freeze includes every accepted racing writer");
    check(frozen->records.size() <= 64 && frozen->overwritten + frozen->records.size() == frozen->last_serial, "racing freeze preserves capacity accounting");
    bool suffix_ok = true;
    for (std::size_t i = 0; i < frozen->records.size(); ++i) {
        const auto& r = frozen->records[i];
        suffix_ok &= r.serial == frozen->overwritten+i+1 && r.fields[1] == (r.fields[0]^0x55u);
    }
    check(suffix_ok, "racing freeze retains consistent suffix");
    check(!raced->push(1, descriptor_event_kind::reset, {}) && !raced->freeze(), "racing freeze rejects all later writes and dumps");
    check(frozen->records.back().serial == frozen->last_serial, "snapshot includes final accepted record");
    std::cout << checks << " checks, " << failures << " failures\n";
    return failures ? 1 : 0;
}
