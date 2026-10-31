"""Private guarded Breach route diagnostic. Ordinary input; no guest writes."""
import sys, json, time, math, argparse
from pathlib import Path
from contextlib import ExitStack
sys.path.insert(0, 'F:/d1/d1xenia/scripts')
from d1_guardian_route import Guard, GuardedPad, GuardedPlayer, SharedTail, HookLease, HOOK, RouteStopped, utc_now, BREACH_SLICE, WALL_SLICE, UNSTABLE_RETRIES, UNSTABLE_RETRY_SECONDS
from d1_walker import Walker, Grid, facing_from_pointers
from d1_traverse import heading
from rpcs3_owned_memory import OwnedMemory, emulator_identity

class BreachGuard(Guard):
    def check(self):
        state = self.observe()
        for _ in range(UNSTABLE_RETRIES):
            if state['stable']: break
            time.sleep(UNSTABLE_RETRY_SECONDS)
            state = self.observe()
        if self.deadline and time.monotonic() >= self.deadline: raise RouteStopped('bounded exploration deadline')
        if not (state['stable'] and state['current']==36 and state['engine'] in (0,3)):
            raise RouteStopped(f'mission is loading/leaving: {state}')
        if self.world.current != BREACH_SLICE or state['native_slice_current'] != BREACH_SLICE:
            raise RouteStopped('no verified current Breach slice')
        if self.world.target not in (None, WALL_SLICE, BREACH_SLICE) or state['native_slice_target'] not in (None, WALL_SLICE, BREACH_SLICE):
            raise RouteStopped('unrelated slice request')
        if self.join_at_start is not None and self.world.joins != self.join_at_start: raise RouteStopped('activity rejoined')
        self.check_player()
        return state

ap=argparse.ArgumentParser()
ap.add_argument('--run', default='JOIN208')
ap.add_argument('--name', required=True)
ap.add_argument('--budget', type=float, default=120)
ap.add_argument('--radius', type=float, default=1)
ap.add_argument('--go', nargs='*', default=[])
ap.add_argument('--face')
ap.add_argument('--hold', nargs='*', default=[])
a=ap.parse_args()
root=Path(__file__).parent
manifest=root.parent/'SIGNON_LOCAL/logs'/f'FINAL124_{a.run}-processes.json'
identity=emulator_identity(json.loads(manifest.read_text(encoding='utf-8-sig')))
out=root/f'{a.run}-{a.name}'
out.mkdir(exist_ok=False)
pad=None
result={'utc':utc_now(),'run':a.run,'scope':'ordinary guarded input; read-only guest memory','status':'stopped'}
with (out/'audit.jsonl').open('x',buffering=1,encoding='utf-8') as stream:
    def audit(**fields): stream.write(json.dumps({'utc':utc_now(),**fields})+'\n')
    def log(message): print(message,flush=True); audit(ev='walker',message=message)
    try:
        with OwnedMemory(manifest) as mem, ExitStack() as cleanup:
            cleanup.callback(lambda:pad.release() if pad else None)
            guard=BreachGuard(mem,SharedTail(manifest.parent/f'FINAL124_{a.run}-ahl.log'),out,audit)
            for _ in range(100):
                guard.observe()
                if guard.tail.offset >= guard.tail.path.stat().st_size: break
            guard.join_at_start=guard.world.joins
            guard.deadline=time.monotonic()+a.budget
            trace=SharedTail(Path(identity['executable']).parent/'log/RPCS3.log',fresh=True)
            cleanup.enter_context(HookLease(Path(identity['executable']).parent/'d1/d1ppu.txt',math.ceil(a.budget*120)+512))
            entity=None
            for _ in range(50):
                guard.check()
                hits=[int(m[1],16) for line in trace.lines() if (m:=HOOK.search(line))]
                if hits: entity=hits[-1]; break
                time.sleep(.1)
            if not entity: raise RouteStopped('no fresh follow-player observation')
            guard.bind_player(trace,entity)
            player=GuardedPlayer(mem,entity,guard)
            facing=facing_from_pointers(mem)
            if facing is None: raise RouteStopped('verified facing unavailable')
            pad=GuardedPad(identity,mem,guard)
            grid=Grid(out/'grid.json')
            walker=Walker(mem,player,pad,(facing,1),grid,log)
            audit(ev='start',identity=identity,entity=hex(entity),xyz=player.position())
            for point in a.go:
                xy=[float(x) for x in point.split(',')]
                if len(xy)!=2 or not all(math.isfinite(x) for x in xy): raise ValueError('bad point')
                audit(ev='goal',xy=xy)
                if not walker.go(xy,radius=a.radius,budget=max(0,guard.deadline-time.monotonic())): raise RouteStopped(f'goal not reached: {xy}')
            if a.face:
                xy=[float(x) for x in a.face.split(',')]
                if not walker.face(heading(player.position(),xy)): raise RouteStopped('camera heading not reached')
            for hold in a.hold:
                keys,ms=hold.split(':')
                ms=int(ms)
                if not 0<ms<=2000: raise ValueError('hold outside bounded 1..2000ms')
                pad.hold(keys.split(','),ms)
                audit(ev='after_input',xyz=player.position())
            result.update(status='completed',xyz=player.position(),entity=hex(entity),yaw=walker.yaw())
            grid.save()
    except (RouteStopped, OSError, ValueError, RuntimeError) as exc:
        result['reason']=str(exc)
    finally:
        result['finished_utc']=utc_now()
        (out/'result.json').write_text(json.dumps(result,indent=2)+'\n')
        print(json.dumps(result),flush=True)
raise SystemExit(0 if result['status']=='completed' else 1)
