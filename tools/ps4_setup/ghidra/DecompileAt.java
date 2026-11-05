// SPDX-License-Identifier: GPL-2.0-or-later
// Print the decompiled C of the function containing each address given as a script argument
// (hex vaddrs, e.g. 0x403140). Used by tools/ps4_setup/d1-ghidra.ps1 -Decompile.
//@category D1
import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;

public class DecompileAt extends GhidraScript {
    @Override
    public void run() throws Exception {
        DecompInterface decompiler = new DecompInterface();
        decompiler.openProgram(currentProgram);
        for (String arg : getScriptArgs()) {
            Address address = toAddr(Long.decode(arg));
            Function function = getFunctionContaining(address);
            if (function == null) {
                function = createFunction(address, null);
            }
            if (function == null) {
                println("// no function at " + arg);
                continue;
            }
            DecompileResults result = decompiler.decompileFunction(function, 180, monitor);
            println("// ==== " + function.getName() + " @ " + function.getEntryPoint() + " (asked " + arg + ")");
            println(result.decompileCompleted()
                ? result.getDecompiledFunction().getC()
                : "// decompile failed: " + result.getErrorMessage());
        }
        decompiler.dispose();
    }
}
