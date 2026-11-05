// SPDX-License-Identifier: GPL-2.0-or-later
// List every instruction with a given scalar or address operand (a struct field offset such as 0x1350, or a call
// target such as 0x4e3910, which finds the callers), with the function it sits in. Arguments: one or more hex values. Used by tools/ps4_setup/d1-ghidra.ps1 -Find.
//@category D1
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.listing.InstructionIterator;
import ghidra.program.model.scalar.Scalar;

import java.util.HashSet;
import java.util.Set;

public class FindScalar extends GhidraScript {
    @Override
    public void run() throws Exception {
        Set<Long> wanted = new HashSet<>();
        for (String arg : getScriptArgs()) {
            wanted.add(Long.decode(arg));
        }
        InstructionIterator it = currentProgram.getListing().getInstructions(true);
        while (it.hasNext() && !monitor.isCancelled()) {
            Instruction ins = it.next();
            for (int i = 0; i < ins.getNumOperands(); i++) {
                for (Object o : ins.getOpObjects(i)) {
                    long v = o instanceof Scalar ? ((Scalar) o).getUnsignedValue() : o instanceof Address ? ((Address) o).getOffset() : -1;
                    if (v != -1 && wanted.contains(v)) {
                        Function f = getFunctionContaining(ins.getAddress());
                        println(ins.getAddress() + "  " + (f == null ? "?" : f.getName() + "@" + f.getEntryPoint())
                            + "  " + ins);
                    }
                }
            }
        }
    }
}
