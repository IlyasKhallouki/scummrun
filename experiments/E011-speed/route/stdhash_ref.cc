// E011 route: the reference for taskcore/src/stdhash.rs. Runs a script of operations on libstdc++'s real unordered
// containers (the ones Fast Downward's landmark code uses: utils::HashSet<FactPair>, std::unordered_set<int>,
// std::unordered_map<int, std::vector<FactPair>>) and prints each container's iteration order when asked; the Rust
// emulation runs the same script (`lama stdhash FILE`) and must print the same bytes.
//
// Script lines: F n (new FactPair set n) | f n var val (insert) | e n var val (erase) | c dst src (copy) |
//   x dst a b (dst = get_intersection(a, b)) | r dst k v1 a1 ... (dst = HashSet(vector)) | p n (print set n) |
//   I n | i n x | j n x (erase) | q n (print int set n) | M n | m n key var val (map[key].push_back) | o n (print map n)
// build: g++ -O2 -std=c++20 -I $DOWNWARD/src/search -o stdhash_ref stdhash_ref.cc
#include "utils/hash.h"
#include <cstdio>
#include <iostream>
#include <map>
#include <sstream>
#include <string>
#include <unordered_map>
#include <unordered_set>
#include <vector>

struct FactPair {
    int var, value;
    FactPair(int v, int x) : var(v), value(x) {}
    bool operator==(const FactPair &o) const { return var == o.var && value == o.value; }
};
namespace utils {
inline void feed(HashState &h, const FactPair &f) {
    feed(h, f.var);
    feed(h, f.value);
}
}

using FSet = utils::HashSet<FactPair>;

int main() {
    std::map<int, FSet> fs;
    std::map<int, std::unordered_set<int>> is;
    std::map<int, std::unordered_map<int, std::vector<FactPair>>> ms;
    std::string line;
    printf("hash00 %llu\n", (unsigned long long)utils::get_hash64(FactPair(0, 0)));
    while (std::getline(std::cin, line)) {
        std::istringstream in(line);
        std::string op;
        in >> op;
        int n, a, b, c;
        if (op == "F") { in >> n; fs[n] = FSet(); }
        else if (op == "f") { in >> n >> a >> b; fs[n].insert(FactPair(a, b)); }
        else if (op == "e") { in >> n >> a >> b; fs[n].erase(FactPair(a, b)); }
        else if (op == "c") { in >> n >> a; FSet copy(fs[a]); fs[n] = copy; }
        else if (op == "x") {
            in >> n >> a >> b;
            FSet r;
            for (const FactPair &f : fs[a]) if (fs[b].count(f)) r.insert(f);
            fs[n] = std::move(r);
        } else if (op == "r") {
            in >> n >> c;
            std::vector<FactPair> v;
            for (int k = 0; k < c; ++k) { in >> a >> b; v.emplace_back(a, b); }
            fs[n] = FSet(v.begin(), v.end());
        } else if (op == "p") {
            in >> n;
            printf("F%d:", n);
            for (const FactPair &f : fs[n]) printf(" %d=%d", f.var, f.value);
            printf(" |%zu\n", fs[n].bucket_count());
        } else if (op == "I") { in >> n; is[n] = std::unordered_set<int>(); }
        else if (op == "i") { in >> n >> a; is[n].insert(a); }
        else if (op == "j") { in >> n >> a; is[n].erase(a); }
        else if (op == "q") {
            in >> n;
            printf("I%d:", n);
            for (int x : is[n]) printf(" %d", x);
            printf(" |%zu\n", is[n].bucket_count());
        } else if (op == "M") { in >> n; ms[n] = std::unordered_map<int, std::vector<FactPair>>(); }
        else if (op == "m") { in >> n >> a >> b >> c; ms[n][a].push_back(FactPair(b, c)); }
        else if (op == "o") {
            in >> n;
            printf("M%d:", n);
            for (auto &[k, v] : ms[n]) printf(" %d/%zu", k, v.size());
            printf(" |%zu\n", ms[n].bucket_count());
        }
    }
}
