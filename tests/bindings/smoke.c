/* C header smoke test: abi version and cfgbin_to_sql on a synthetic fixture. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "nie_contract.h"

int main(int argc, char **argv) {
    if (argc < 2 || nie_ffi_abi_version() != NIE_FFI_ABI_VERSION) return 2;
    FILE *f = fopen(argv[1], "rb");
    if (!f) return 3;
    uint8_t buf[4096];
    size_t n = fread(buf, 1, sizeof buf, f);
    fclose(f);
    char *sql = nie_cfgbin_to_sql(buf, n, 1);
    if (!sql || !strstr(sql, "CREATE TABLE") || !strstr(sql, "ROOT")) return 4;
    size_t len = strlen(sql);
    nie_string_free(sql);
    if (nie_cfgbin_to_sql((const uint8_t *)"nope", 4, 1) != NULL) return 5;
    printf("c ok %u %zu\n", nie_ffi_abi_version(), len);
    return 0;
}
