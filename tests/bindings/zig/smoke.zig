// Zig binding smoke test: abiVersion and cfgbinToSql on a synthetic fixture.
const std = @import("std");
const nie = @import("nie_contract");

extern fn getenv(name: [*:0]const u8) ?[*:0]const u8;
extern fn fopen(path: [*:0]const u8, mode: [*:0]const u8) ?*anyopaque;
extern fn fread(ptr: [*]u8, size: usize, n: usize, file: *anyopaque) usize;
extern fn fclose(file: *anyopaque) c_int;

pub fn main() !void {
    const allocator = std.heap.c_allocator;
    if (nie.abiVersion() != nie.abi_version_expected) return error.AbiMismatch;
    const fixture = getenv("NIE_FIXTURE") orelse return error.NoFixture;
    const file = fopen(fixture, "rb") orelse return error.NoFixtureFile;
    defer _ = fclose(file);
    var buf: [4096]u8 = undefined;
    const n = fread(&buf, 1, buf.len, file);
    const sql = (try nie.cfgbinToSql(allocator, buf[0..n], 1)) orelse return error.NoSql;
    defer allocator.free(sql);
    if (std.mem.indexOf(u8, sql, "CREATE TABLE") == null) return error.BadSql;
    if ((try nie.cfgbinToSql(allocator, "nope", 1)) != null) return error.GarbageAccepted;
    std.debug.print("zig ok {d} {d}\n", .{ nie.abiVersion(), sql.len });
}
