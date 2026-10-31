"""Private, offline replica of the owned build's power curve operations.

No guest calls, process attachment, writes to the lab, or inferred multipliers.
Offsets and operations come from 528f90/529c40/237710 in SHA-pinned image.
This evaluates only finite, positive, equal-triple power inputs and the seven
absent CVar overrides measured in damage-cvars-native.json.
"""
from pathlib import Path
import struct, json
P = Path(__file__).parent
B = (P/'owned-80b1a005.bin').read_bytes()
def f(x): return struct.unpack('<f', struct.pack('<f', x))[0]
def unpack(fmt, off): return struct.unpack_from('<'+fmt, B, off)
def splat(x): return [f(x)]*4
def poly(c, x):
    return [f(f(f(f(x[i]*c[0][i])+c[1][i])*f(x[i]*x[i])) + f(f(x[i]*c[2][i])+c[3][i])) for i in range(4)]
def piece(c, x, previous=None):
    """Native masks pick the last satisfied threshold (ascending knots)."""
    v = poly(c[:4], x)
    thresholds = c[4]
    assert all(thresholds[i] <= thresholds[i+1] for i in range(3))
    # The observed bytecode splats every polynomial input; reject other forms.
    assert x == [x[0]]*4
    indexes = [i for i in range(4) if thresholds[i] <= x[0]]
    return splat(v[indexes[-1]]) if indexes else (previous or splat(0))
def eval_curve(off, coordinate):
    n, relative, nc, crel = unpack('QqQq', off)
    assert n < 128 and nc < 128 and unpack('I',off+0x2c)[0] == 0
    code = B[off+8+relative+16:off+8+relative+16+n]
    const = [list(unpack('4f',off+0x18+crel+16+i*16)) for i in range(nc)]
    inputs = [splat(coordinate), splat(0)]
    stack=[]; out={}; pc=0
    while pc<len(code):
        op=code[pc]; pc+=1
        if op in (0x22,0x34,0x37,0x38,0x39,0x3c,0x3e):
            k=code[pc]; pc+=1
        if op==0x34: stack.append(const[k][:])
        elif op==0x3c: stack.append(inputs[k][:])
        elif op==0x3e: out[k]=stack.pop()
        elif op==0x22: stack[-1]=[stack[-1][(k>>s)&3] for s in (6,4,2,0)]
        elif op==0x21: stack[-1]=splat(stack[-1][0])
        elif op==0x23: stack[-1]=[min(1,max(0,v)) for v in stack[-1]]
        elif op==0x37: stack[-1]=piece(const[k:k+5],stack[-1])
        elif op in (0x38,0x39):
            x=stack.pop(); previous=stack.pop() if op==0x39 else splat(0)
            first=piece(const[k:k+4]+[const[k+8]], x, previous)
            stack.append(piece(const[k+4:k+8]+[const[k+9]],x,first))
        elif op==0x03:
            b=stack.pop(); a=stack.pop(); stack.append([f(x*y) for x,y in zip(a,b)])
        elif op==0x10:
            t=stack.pop(); b=stack.pop(); a=stack.pop()
            stack.append([f(x+f(z*f(y-x))) for x,y,z in zip(a,b,t)])
        else: raise ValueError(hex(op))
    assert not stack and set(out)=={0} and out[0]==[out[0][0]]*4
    return out[0][0]
def coordinate(attacker, defender, table):
    lower, upper=unpack('2f', table)
    # 528f90: clamp attacker to <= defender-lower, then defender to
    # <= (clamped attacker)+upper. Outputs R9=attacker, stack=defender.
    a=f(min(attacker,f(defender-lower)))
    d=f(min(defender,f(a+upper)))
    count,relative=unpack('Qq',table+8)
    knots=[unpack('3f',table+0x20+relative+i*12) for i in range(count)]
    prev=(0.,0.,0.); lo=hi=0.
    for row in knots:
        if a>=row[0]: prev=row; lo,hi=row[1:]; continue
        gap=f(row[0]-prev[0])
        if gap>0:
            t=f(f(a-prev[0])/gap)
            lo=f(prev[1]+f(t*f(row[1]-prev[1])))
            hi=f(prev[2]+f(t*f(row[2]-prev[2])))
        break
    value=0 if hi<=f(lo+1.1920928955078125e-7) else min(1,max(0,f(f(f(d-a)-lo)/f(hi-lo))))
    return {'attacker_clamped':a,'defender_clamped':d,'interval':[lo,hi],'coordinate':value}
results=[]
for player in (1.,2.,5.,40.):
    for incoming,table in ((False,0xe8),(True,0x190)):
        c=coordinate(1. if incoming else player,player if incoming else 1.,table)
        results.append({'player_power':player,'npc_power':1.,'direction':'incoming' if incoming else 'outgoing',**c,
          'channels':[eval_curve(table+offset,c['coordinate']) for offset in (0x18,0x48,0x78)]})
# Independent endpoint controls from constant vectors: exact native selections.
assert eval_curve(0x100,0)==1.0
assert abs(eval_curve(0x1a8,1)-1)<1e-6
assert all(abs(x['channels'][0]-1)<1e-6 for x in results if x['player_power']==40.)
out={'method':'offline float32 replica of owned native selector and expression bytecode; not a live hit test',
     'scope':'positive finite powers; equal triples; observed absent overrides; channel 0 is damage multiplier',
     'results':results,'curve_samples':{hex(off):[[x,eval_curve(off,x)] for x in (0,.25,.5,.75,1)] for off in (0x100,0x1a8)}}
(P/'damage-power-curve-results.json').write_text(json.dumps(out,indent=2))
print(json.dumps(out,indent=2))
