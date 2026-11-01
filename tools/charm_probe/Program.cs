// SPDX-License-Identifier: GPL-3.0-or-later
// Console probe that drives MontagueM's Charm "Tiger" library (GPL-3.0) headless
// against the Destiny 1 PS4 56578 packages. Run with the working directory set to a
// staging folder: Tiger writes ./config.json and ./PackagePathsCaches there.
//
//   dotnet CharmProbe.dll <dump>/CUSA00219/packages <out dir> [--no-investment] [--scan]
//
// Writes named-tags.tsv, activities-bubbles.tsv and inventory-items.tsv: game data, keep it
// outside the repository.
using System.Reflection;
using Arithmic;
using Tiger;
using Tiger.Schema;
using Tiger.Schema.Investment;

if (args.Contains("--scan")) { FieldScan.Run(); return; }
if (args.Length < 1 || args[0].StartsWith("--"))
{
    Console.Error.WriteLine("usage: CharmProbe <packages dir> [out dir] [--no-investment] [--scan]");
    return;
}
string pkgDir = args[0];
string outDir = args.Length > 1 && !args[1].StartsWith("--") ? args[1] : Directory.GetCurrentDirectory();
bool doInvestment = !args.Contains("--no-investment");
Directory.CreateDirectory(outDir);

var logPath = Path.Combine(outDir, "tiger-log.txt");
var logWriter = new StreamWriter(logPath) { AutoFlush = true };
Log.BindDelegate((_, e) => { lock (logWriter) logWriter.WriteLine(e.Message); });

// --- replicate Charm MainWindow.InitialiseStrategistSingletons -----------------
var asms = AppDomain.CurrentDomain.GetAssemblies();
var lazy = asms.SelectMany(a => a.GetTypes())
    .Select(t => t.GetNonGenericParent(typeof(Strategy.LazyStrategistSingleton<>)))
    .Where(t => t is { ContainsGenericParameters: false })
    .Select(t => t.GetNonGenericParent(typeof(Strategy.StrategistSingleton<>)))
    .ToHashSet();
var all = asms.SelectMany(a => a.GetTypes())
    .Select(t => t.GetNonGenericParent(typeof(Strategy.StrategistSingleton<>)))
    .Where(t => t is { ContainsGenericParameters: false })
    .ToHashSet();
all.ExceptWith(lazy);
foreach (var t in SortByInit(all.ToList()))
{
    Console.WriteLine($"register {t.GenericTypeArguments[0].Name}");
    t.GetMethod("RegisterEvents", BindingFlags.Static | BindingFlags.NonPublic)!.Invoke(null, null);
}

// --- config + subsystems (ConfigSubsystem.LoadConfig sets the strategy) --------
File.WriteAllText("config.json",
    "{\"Common\":{\"PackagesPath\":{\"DESTINY1_RISE_OF_IRON\":" +
    Newtonsoft.Json.JsonConvert.ToString(pkgDir) +
    "},\"CurrentStrategy\":1000,\"ExportPath\":" + Newtonsoft.Json.JsonConvert.ToString(Path.Combine(outDir, "export")) + "}}");
CharmInstance.Args = new CharmArgs(new[] { "probe", "-strategy=DESTINY1_RISE_OF_IRON" });
var sw = System.Diagnostics.Stopwatch.StartNew();
CharmInstance.InitialiseSubsystems();
Console.WriteLine($"strategy={Strategy.CurrentStrategy} init {sw.Elapsed.TotalSeconds:F1}s");

var pr = PackageResourcer.Get();
// CacheAllD1NamedTags is async void; wait for it to settle.
int last = -1;
for (int i = 0; i < 120; i++)
{
    Thread.Sleep(1000);
    int n = pr.GetD1Activities().Count;
    if (n > 0 && n == last) break;
    last = n;
}
var named = pr.GetD1Activities();
Console.WriteLine($"named tags: {named.Count}");

using (var w = new StreamWriter(Path.Combine(outDir, "named-tags.tsv")))
{
    w.WriteLine("tag\tclass\tname");
    foreach (var (hash, cls) in named.OrderBy(k => k.Value.ToString()).ThenBy(k => k.Key.ToString()))
        w.WriteLine($"{hash}\t{cls}\t{pr.GetActivityName(hash)}");
}
foreach (var g in named.GroupBy(k => k.Value.ToString()).OrderByDescending(g => g.Count()))
    Console.WriteLine($"  class {g.Key}: {g.Count()}  e.g. {string.Join(" | ", g.Take(3).Select(k => pr.GetActivityName(k.Key)))}");

// --- activities: bubble names via Charm's D1 Activity schema --------------------
using (var w = new StreamWriter(Path.Combine(outDir, "activities-bubbles.tsv")))
{
    int ok = 0, fail = 0;
    foreach (var (hash, cls) in named.Where(k => k.Value.ToString().Equals("2E058080", StringComparison.OrdinalIgnoreCase)))
    {
        try
        {
            var act = FileResourcer.Get().GetFile<Tiger.Schema.Activity.DESTINY1_RISE_OF_IRON.Activity>(hash);
            var bubbles = act.EnumerateBubbles().Select(b => $"{b.Name}={b.ChildMapReference?.Hash}").ToList();
            w.WriteLine($"{hash}\t{pr.GetActivityName(hash)}\t{string.Join("; ", bubbles)}");
            ok++;
        }
        catch (Exception e) { w.WriteLine($"{hash}\t{pr.GetActivityName(hash)}\tERROR {e.GetType().Name}: {e.Message}"); fail++; }
    }
    Console.WriteLine($"activities parsed ok={ok} fail={fail}");
}

// --- investment (inventory items) ----------------------------------------------
if (doInvestment)
{
    try
    {
        sw.Restart();
        Investment.LazyInit();
        var inv = Investment.Get();
        int count = inv.InventoryItemStringThings?.Count ?? 0;
        Console.WriteLine($"investment init {sw.Elapsed.TotalSeconds:F1}s; item string entries={count}");
        using var w = new StreamWriter(Path.Combine(outDir, "inventory-items.tsv"));
        w.WriteLine("index\titemHash\tname");
        int ok = 0, fail = 0;
        for (int i = 0; i < count; i++)
        {
            try
            {
                var item = inv.GetInventoryItem(i);
                w.WriteLine($"{i}\t{item.TagData.InventoryItemHash.Hash32}\t{inv.GetItemName(item)}");
                ok++;
            }
            catch (Exception e) { if (fail++ < 5) Console.WriteLine($"  item {i}: {e.GetType().Name}: {e.Message}"); }
        }
        Console.WriteLine($"inventory items ok={ok} fail={fail}");
    }
    catch (Exception e) { Console.WriteLine($"investment FAILED: {e}"); }
}

Log.Info("probe done");
Console.WriteLine($"outputs in {outDir}; log {logPath}");
Environment.Exit(0);

static IEnumerable<Type> SortByInit(List<Type> types)
{
    var map = new Dictionary<Type, List<Type>>();
    var cnt = new Dictionary<Type, int>();
    foreach (var type in types)
        foreach (InitializeAfterAttribute a in type.GenericTypeArguments[0].GetCustomAttributes(typeof(InitializeAfterAttribute), true))
        {
            var dep = a.TypeToInitializeAfter.GetNonGenericParent(typeof(Strategy.StrategistSingleton<>))!;
            if (!map.ContainsKey(dep)) { map[dep] = new(); cnt[dep] = 0; }
            map[dep].Add(type);
            cnt[type] = cnt.ContainsKey(type) ? cnt[type] + 1 : 1;
        }
    var sorted = types.Where(t => !cnt.ContainsKey(t)).ToList();
    var q = new Queue<Type>(map.Keys.Where(k => cnt[k] == 0));
    while (q.Count > 0)
    {
        var t = q.Dequeue();
        sorted.Add(t);
        if (map.TryGetValue(t, out var deps))
            foreach (var d in deps) if (--cnt[d] == 0) q.Enqueue(d);
    }
    return sorted;
}

