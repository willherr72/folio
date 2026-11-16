import {useState} from "react";
import {act,cleanup,fireEvent,render,screen} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {PreciseNumberInput} from "../src/components/PreciseNumberInput";

afterEach(cleanup);
it("selects the expanded exact draft before the caller's keyboard-focus handler",()=>{
 const values:string[]=[],changed=vi.fn();
 render(<PreciseNumberInput aria-label="Height" value="40.487854" onChange={changed} onFocus={e=>values.push('focus:'+e.currentTarget.value)}/>);
 const input=screen.getByRole('spinbutton') as HTMLInputElement;
 // jsdom exposes no number-input offsets and queues its select event. Observe
 // the synchronous native operation; actual replacement is tested in Windows.
 vi.spyOn(input,'select').mockImplementation(()=>{values.push('select:'+input.value);HTMLInputElement.prototype.select.call(input);});
 act(()=>input.focus());
 expect(values).toEqual(['select:40.487854','focus:40.487854']);
 expect(changed).not.toHaveBeenCalled();
});
it("preserves pointer focus and composes pointer handlers without forcing selection",()=>{
 const down=vi.fn(),up=vi.fn(),focus=vi.fn();
 render(<PreciseNumberInput aria-label="Height" value="40.487854" onChange={()=>{}} onFocus={focus} onPointerDown={down} onPointerUp={up}/>);
 const input=screen.getByRole('spinbutton') as HTMLInputElement,selected=vi.spyOn(input,'select');
 fireEvent.pointerDown(input);act(()=>input.focus());fireEvent.pointerUp(input);
 expect(input).toHaveValue(40.487854);expect(selected).not.toHaveBeenCalled();expect(down).toHaveBeenCalledOnce();expect(up).toHaveBeenCalledOnce();expect(focus).toHaveBeenCalledOnce();
 act(()=>input.blur());act(()=>input.focus());expect(selected).toHaveBeenCalledOnce();
});
it.each(['pointerUp','pointerCancel','preventDefault'] as const)("clears unused pointer intent after %s before keyboard focus",mode=>{
 const cancel=vi.fn();
 render(<PreciseNumberInput aria-label="Height" value="40.487854" onChange={()=>{}} onPointerDown={e=>{if(mode==='preventDefault')e.preventDefault();}} onPointerCancel={cancel}/>);
 const input=screen.getByRole('spinbutton') as HTMLInputElement,selected=vi.spyOn(input,'select');
 fireEvent.pointerDown(input);
 if(mode==='pointerUp')fireEvent.pointerUp(input);
 if(mode==='pointerCancel')fireEvent.pointerCancel(input);
 act(()=>input.focus());expect(selected).toHaveBeenCalledOnce();
 expect(cancel).toHaveBeenCalledTimes(mode==='pointerCancel'?1:0);
});
it("does not mistake ancestor-cancelled pointer focus for a later keyboard entry",()=>{
 render(<div onPointerDown={event=>event.preventDefault()}><PreciseNumberInput aria-label="Height" value="40.487854" onChange={()=>{}}/></div>);
 const input=screen.getByRole('spinbutton') as HTMLInputElement,selected=vi.spyOn(input,'select');
 fireEvent.pointerDown(input);fireEvent.pointerUp(document.body);
 act(()=>input.focus());expect(selected).toHaveBeenCalledOnce();
});
it("reveals the exact value before native focus returns so replacement selects the full draft",()=>{
 render(<PreciseNumberInput aria-label="Width" value="267.04565" onChange={()=>{}}/>);
 const input=screen.getByRole("spinbutton");expect(input).toHaveValue(267.046);
 act(()=>{input.focus();expect(input).toHaveValue(267.04565);});
});
it("compacts idle values, reveals exact precision on focus, and emits nothing for focus/blur",()=>{
 const changed=vi.fn(),focus=vi.fn(),blur=vi.fn();
 render(<PreciseNumberInput aria-label="Left" value="143.99998474121094" onChange={changed} onFocus={focus} onBlur={blur}/>);
 const input=screen.getByRole("spinbutton");expect(input).toHaveValue(144);
 fireEvent.focus(input);expect(input).toHaveValue(143.99998474121094);
 fireEvent.blur(input);expect(input).toHaveValue(144);expect(changed).not.toHaveBeenCalled();expect(focus).toHaveBeenCalledOnce();expect(blur).toHaveBeenCalledOnce();
});
it("keeps a precise edit through blur/refocus and leaves cleared drafts empty",()=>{
 const changed=vi.fn();function Field(){const [value,setValue]=useState("1.25");return <PreciseNumberInput aria-label="Width" value={value} onChange={e=>{changed(e.target.value);setValue(e.target.value);}}/>;}
 render(<Field/>);const input=screen.getByRole("spinbutton");fireEvent.focus(input);fireEvent.change(input,{target:{value:"1.23456789"}});expect(input).toHaveValue(1.23456789);
 fireEvent.blur(input);expect(input).toHaveValue(1.235);fireEvent.focus(input);expect(input).toHaveValue(1.23456789);expect(changed.mock.calls).toEqual([["1.23456789"]]);
 fireEvent.change(input,{target:{value:""}});fireEvent.blur(input);expect(input).toHaveValue(null);
});
it.each(["0.00004","-0.00004","0.00000001"])("does not turn tiny nonzero %s into displayed zero",value=>{
 render(<PreciseNumberInput aria-label="Position" readOnly value={value}/>);expect(screen.getByRole("spinbutton")).toHaveValue(Number(value));
});
it.each([{value:"0.0099999",min:0.01},{value:"300.0000001",max:300},{value:"2.9999",max:2.99995}])("does not round across a native numeric bound: $value",props=>{
 render(<PreciseNumberInput aria-label="Size" readOnly {...props}/>);expect(screen.getByRole("spinbutton")).toHaveValue(Number(props.value));
});
it("updates exact external/reset values while focused and compacts only after blur",()=>{
 const {rerender}=render(<PreciseNumberInput aria-label="Left" readOnly value="14.123456"/>);const input=screen.getByRole("spinbutton");fireEvent.focus(input);
 rerender(<PreciseNumberInput aria-label="Left" readOnly value="20.987654"/>);expect(input).toHaveValue(20.987654);fireEvent.blur(input);expect(input).toHaveValue(20.988);
});
