// C# P/Invoke binding smoke test: abi version and cfgbin_to_sql on a synthetic fixture.
using System;
using System.IO;

if (Nie.Api.AbiVersion() != Nie.Api.AbiVersionExpected) return 2;
var sql = Nie.Api.CfgbinToSql(File.ReadAllBytes(args[0]), 1);
if (sql is null || !sql.Contains("CREATE TABLE") || !sql.Contains("ROOT")) return 4;
if (Nie.Api.CfgbinToSql(new byte[] { 110, 111, 112, 101 }, 1) is not null) return 5;
Console.WriteLine($"csharp ok {Nie.Api.AbiVersion()} {sql.Length}");
return 0;
