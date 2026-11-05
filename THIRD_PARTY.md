# Third-party research tools

The native-settings investigation used locally installed copies of these projects to inspect
the user's owned content. Their tools, extracted outputs and game material are not redistributed
in this repository. The settings generator is project code derived from the native layout.

- [tiger-pkg](https://github.com/v4nguard/tiger-pkg): package inspection and extraction.
- [Blam Network tiger](https://github.com/Blam-Network/tiger): package-format reference.
- [quicktag](https://github.com/v4nguard/quicktag): tag-format research reference.
- [Ghidra](https://github.com/NationalSecurityAgency/ghidra) (Apache-2.0), run headless on a
  [Temurin](https://adoptium.net/) JDK 25: decompiling the owned eboot (`tools/ps4_setup/d1-ghidra.ps1`);
  its project, which holds the game's code, stays outside the repository.

Each upstream project retains its own licence. Consult that project's licence before copying
or redistributing its implementation; these links do not grant additional rights.
