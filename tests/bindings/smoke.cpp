// C++ header smoke test: abi_version and cfgbin_to_sql on a synthetic fixture.
#include <cstdio>
#include <fstream>
#include <iterator>
#include <vector>
#include "nie_contract.hpp"

int main(int argc, char **argv) {
    if (argc < 2 || nie::abi_version() != NIE_FFI_ABI_VERSION) return 2;
    std::ifstream in(argv[1], std::ios::binary);
    std::vector<std::uint8_t> data((std::istreambuf_iterator<char>(in)), std::istreambuf_iterator<char>());
    auto sql = nie::cfgbin_to_sql(data, 1);
    if (!sql || sql->find("CREATE TABLE") == std::string::npos || sql->find("ROOT") == std::string::npos) return 4;
    const std::uint8_t junk[] = {'n', 'o', 'p', 'e'};
    if (nie::cfgbin_to_sql(junk, 1)) return 5;
    std::printf("cpp ok %u %zu\n", nie::abi_version(), sql->size());
    return 0;
}
