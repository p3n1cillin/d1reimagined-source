// SPDX-License-Identifier: GPL-3.0-or-later
// --scan: list schema fields whose [SchemaField] attributes are all for Destiny 2 strategies
// (the fields that crashed upstream Charm's D1 strategy; see patches/charm-d1-strategy-null-guard.patch).
using System.Reflection;
using Tiger;
public static class FieldScan {
  public static void Run() {
    foreach (var t in typeof(TigerFile).Assembly.GetTypes())
      foreach (var f in t.GetFields()) {
        var a = f.GetCustomAttributes<SchemaFieldAttribute>().ToArray();
        if (a.Length > 1 && !a.Any(x => x.Strategy <= TigerStrategy.DESTINY1_RISE_OF_IRON))
          Console.WriteLine($"{t.FullName}.{f.Name}: {string.Join(",", a.Select(x => x.Strategy))}");
      }
  }
}
