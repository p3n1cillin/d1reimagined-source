# charm_probe

A console probe for build 56578's packages. It uses the `Tiger` library from MontagueM's
[Charm](https://github.com/MontagueM/Charm) (GPL-3.0), so these files are GPL-3.0-or-later. See
[`notes/charm-56578.md`](../../notes/charm-56578.md) for what it found.

1. Clone Charm, then apply [`patches/charm-d1-strategy-null-guard.patch`](../../patches/charm-d1-strategy-null-guard.patch)
   with `git apply`.
2. Build:

   ```
   dotnet build tools/charm_probe/CharmProbe.csproj -c Release -p:CharmDir=<charm> -o <bin>
   ```

3. From an empty working folder outside the repo (Tiger writes `config.json` and caches there), run:

   ```
   dotnet <bin>/CharmProbe.dll <dump>/CUSA00219/packages <out dir> [--no-investment]
   ```

   `--scan` instead lists the D2-only schema fields that the patch guards against.

It needs the .NET 7 SDK. The outputs (`named-tags.tsv`, `activities-bubbles.tsv`,
`inventory-items.tsv`) are game data: keep them outside the repository.
