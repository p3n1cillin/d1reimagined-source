from pathlib import Path
repo=Path(r'F:\d1\public-work\d1reimagined-source')
for file in ('docs/STATUS.md','notes/ps4-56578-continuation-2026-10-27.md'):
    p=repo/file; text=p.read_text(encoding='utf-8')
    if file.startswith('notes'):
        before,sep,tail=text.partition('## Divide objective handover corrected')
    else: before,sep,tail='','',text
    replacements={'objective14':'objective 14','objective17':'objective 17','serving17':'serving 17','mode3':'mode 3','mode5':'mode 5','mode1':'mode 1',' at3':' at 3',' at5':' at 5',' at13':' at 13',' to0':' to 0',' serving1':' serving 1',' to3':' to 3','kind0x':'kind 0x','indices46':'indices 46','stride0x':'stride 0x','two24-byte':'two 24-byte','selector0':'selector 0','selector1':'selector 1','On56578':'On 56578','example':'example','near(266':'near (266','near(309':'near (309','**40 passed,7 failed**':'**40 passed, 7 failed**','Owned56578':'Owned 56578','slice96':'slice 96','about17':'about 17'}
    for a,b in replacements.items(): tail=tail.replace(a,b)
    p.write_text(before+sep+tail,encoding='utf-8',newline='\n')
body='''## Problem and change

On PS4 build 56578, the Divide handover left “Fight Through the Wall” visible and treated mode 3 as completion. Clearing objective 14 to 0 and presenting 17 in mode 3, with marker 46 and local machine 12, now shows “Fight Through the Fallen”, its authored description and a yard waypoint without a new-mission banner.

Add `d1-oracle sensor objective` and synthetic controls to generate the handover while preserving authority and other clients. Fix a separate route-loop regression where arrival tolerance and off-route recovery alternated without sending movement. Update the public checkpoint and journal, including the authored squad positions that narrow the remaining encounter investigation.

## Validation

- Focused Rust objective control/test passed; release oracle build passed.
- 189 Python script tests passed, including an execution of the real route loop at the failing synthetic geometry.
- Native divide16: Rust-generated list accepted, objective fields read back, resolver traced to the authored point, correct title/description/waypoint inspected. A reset to 0 then mode 3 also worked without mode 1. The full unattended mission steps do not yet include this handover.
- Native route retry sent movement beyond the former busy-loop point; the Divide route and combat are not complete.
- Broader oracle library run: 40 passed, 7 failed. Six require the absent reference executable; one requires a build-specific item table not generated in the source checkout. The existing 56578 sensor tests passed. No substitute reference inputs were used.

## Inputs and attribution

Original Rust/Python changes retain existing GPL notices. New controls use synthetic data. Only factual findings and original source are included; owned packages/executables, generated lists, screenshots, keys and raw run evidence stay private. Authored placement facts were checked against the user's owned 56578 content and loaded native bytes.
'''
Path(__file__).with_name('pr-body.md').write_text(body,encoding='utf-8')
