//! Insert canonical fragments at the checked source span without painting its
//! prefix again. The prefix is replayed only to restore state, paths and cursor.
use super::*;
type Matrix=[f64;6];
const IDENTITY:Matrix=[1.,0.,0.,1.,0.,0.];

fn matrix(op:&Operation)->EngineResult<Matrix>{
 if op.operands.len()!=6{return Err(invalid("Malformed paragraph transform."));}
 let mut result=[0.;6];for (out,value) in result.iter_mut().zip(&op.operands){*out=number(value)? as f64;}
 Ok(result)
}
fn product(a:Matrix,b:Matrix)->EngineResult<Matrix>{
 let result=[a[0]*b[0]+a[2]*b[1],a[1]*b[0]+a[3]*b[1],a[0]*b[2]+a[2]*b[3],a[1]*b[2]+a[3]*b[3],a[0]*b[4]+a[2]*b[5]+a[4],a[1]*b[4]+a[3]*b[5]+a[5]];
 if result.iter().any(|v|!v.is_finite()||!(*v as f32).is_finite()){return Err(invalid("Paragraph transforms exceed their finite bounds."));}Ok(result)
}
fn operand(op:&Operation,index:usize,count:usize)->EngineResult<f64>{
 if op.operands.len()!=count{return Err(invalid("Malformed paragraph state."));}Ok(number(&op.operands[index])? as f64)
}
#[derive(Clone)]
struct State{ctm:Matrix,size:f64,scale:f64,rise:f64,leading:f64,mode:f64,clipped:bool}
impl Default for State{fn default()->Self{Self{ctm:IDENTITY,size:0.,scale:100.,rise:0.,leading:0.,mode:0.,clipped:false}}}
struct Context{state:State,stack:Vec<State>,line:Matrix,in_text:bool,text_start:Option<usize>,path_start:Option<usize>,pending_clip:bool}
impl Context{
 fn read(prefix:&[Operation])->EngineResult<Self>{
  let mut c=Self{state:State::default(),stack:vec![],line:IDENTITY,in_text:false,text_start:None,path_start:None,pending_clip:false};
  for (index,op) in prefix.iter().enumerate(){match op.operator.as_str(){
   "q"=>{if !op.operands.is_empty()||c.stack.len()>=65{return Err(invalid("Paragraph state nesting exceeds its limit."));}c.stack.push(c.state.clone());},
   "Q"=>{if !op.operands.is_empty(){return Err(invalid("Malformed paragraph graphics restore."));}c.state=c.stack.pop().ok_or_else(||invalid("Unbalanced paragraph graphics state."))?;},
   "cm"=>c.state.ctm=product(c.state.ctm,matrix(op)?)?,
   "BT"=>{if c.in_text||!op.operands.is_empty(){return Err(invalid("Malformed paragraph text object."));}c.in_text=true;c.line=IDENTITY;c.text_start=Some(index);c.pending_clip=false;},
   "ET"=>{if !c.in_text||!op.operands.is_empty(){return Err(invalid("Unbalanced paragraph text object."));}c.in_text=false;c.text_start=None;c.state.clipped|=c.pending_clip;c.pending_clip=false;},
   "Tf"=>c.state.size=operand(op,1,2)?,
   "Tz"=>c.state.scale=operand(op,0,1)?,
   "Ts"=>c.state.rise=operand(op,0,1)?,
   "TL"=>c.state.leading=operand(op,0,1)?,
   "Tr"=>c.state.mode=operand(op,0,1)?,
   "Tm"=>{if !c.in_text{return Err(invalid("Paragraph matrix outside text."));}c.line=matrix(op)?;},
   "Td"|"TD"=>{
    if !c.in_text{return Err(invalid("Paragraph line movement outside text."));}
    let x=operand(op,0,2)?;let y=operand(op,1,2)?;if op.operator=="TD"{c.state.leading=-y;}
    c.line=product(c.line,[1.,0.,0.,1.,x,y])?;
   },
   "T*"|"'"|"\""=>{
    if !c.in_text{return Err(invalid("Paragraph line movement outside text."));}
    if op.operator=="T*"&&!op.operands.is_empty(){return Err(invalid("Malformed paragraph line movement."));}
    c.line=product(c.line,[1.,0.,0.,1.,0.,-c.state.leading])?;
   },
   "Tj"|"TJ"=>{},
   "m"|"l"|"c"|"v"|"y"|"h"|"re"=>{c.path_start.get_or_insert(index);},
   "S"|"s"|"f"|"F"|"f*"|"B"|"B*"|"b"|"b*"|"n"=>c.path_start=None,
   _=>{},
  }
  if matches!(op.operator.as_str(),"Tj"|"TJ"|"'"|"\"")&&c.state.mode>=4.&&!positioned_text::cursor_only(op){c.pending_clip=true;}
  }
  // Clipping is applied at ET and belongs to the saved graphics state. A Q can
  // fully discard an earlier clip; only a still-live clip is unreconstructible.
  if c.state.clipped||c.pending_clip{return Err(invalid("Clipping text cannot be reconstructed for paragraph editing."));}
  Ok(c)
 }
 fn cursor(&self,source:&RunSnapshot,leading_adjustment:f64)->EngineResult<Operation>{
  let combined=product(self.state.ctm,self.line)?;
  let predicted=[combined[0]*self.state.scale/100.,combined[1]*self.state.scale/100.,combined[2],combined[3]];
  if !close_values(&predicted.map(|n|n as f32),&source.matrix[..4])||self.state.size<=0.||self.state.scale==0.
   ||(self.state.size-source.font_size as f64).abs()>EPSILON as f64{return Err(invalid("The paragraph cursor transform could not be verified."));}
  let determinant=combined[0]*combined[3]-combined[1]*combined[2];
  if determinant==0.||!determinant.is_finite(){return Err(invalid("The paragraph line matrix is singular."));}
  let dx=source.matrix[4] as f64-combined[4];let dy=source.matrix[5] as f64-combined[5];
  let x=(combined[3]*dx-combined[2]*dy)/determinant;
  let y=(-combined[1]*dx+combined[0]*dy)/determinant;
  if (y-self.state.rise).abs()>EPSILON as f64{return Err(invalid("The paragraph line origin could not be verified."));}
  // The native object's origin follows a TJ array's leading numbers. Resume
  // before those numbers: the complete cursor-only removal still includes them.
  let advance=(-x*1000./(self.state.size*self.state.scale/100.)-leading_adjustment) as f32;
  if !advance.is_finite(){return Err(invalid("The paragraph cursor exceeds its limit."));}
  Ok(Operation::new("TJ",vec![Object::Array(vec![Object::Real(advance)])]))
 }
}

/// No text show or drawing is replayed. Line-changing quote operators retain
/// their side effects; painting consumes the current path even when invisible.
fn replay(prefix:&[Operation],context:&Context)->EngineResult<Vec<Operation>>{
 // Only fully restored scopes are disposable. A path or pending W can escape
 // Q and establish a clip later; its contributing scope must survive replay
 // even after the path is consumed and no longer appears in Context::path_start.
 // Dropping other closed histories bounds repeated paragraph edits.
 let cutoff=context.text_start.unwrap_or(prefix.len()).min(context.path_start.unwrap_or(prefix.len()));
 let mut scopes=Vec::new();let mut discard=vec![false;prefix.len()];
 let mut path_update=None;let mut pending_clip=None;
 for (index,op) in prefix.iter().enumerate(){match op.operator.as_str(){
  "q"=>scopes.push((index,path_update,pending_clip)),
  "Q"=>{
   let (start,incoming_path,incoming_clip)=scopes.pop().ok_or_else(||invalid("Unbalanced paragraph replay scope."))?;
   let escaped=path_update.is_some_and(|last|last>=start)||pending_clip.is_some_and(|last|last>=start);
   // Consuming an incoming path/pending W also escapes graphics restoration:
   // removing that consumption would leave it live in the reconstructed tail.
   let consumed=incoming_path.is_some()&&incoming_path!=path_update
       ||incoming_clip.is_some()&&incoming_clip!=pending_clip;
   if index<cutoff&&!escaped&&!consumed{discard[start..=index].fill(true);}
  },
  "m"|"l"|"c"|"v"|"y"|"h"|"re"=>path_update=Some(index),
  "W"|"W*"=>pending_clip=Some(index),
  "S"|"s"|"f"|"F"|"f*"|"B"|"B*"|"b"|"b*"|"n"=>{path_update=None;pending_clip=None;},
  _=>{},
 }}
 let mut output=Vec::new();
 for (index,op) in prefix.iter().enumerate(){if discard[index]{continue;}match op.operator.as_str(){
  "Tj"|"TJ"|"Do"|"sh"|"BI"|"ID"|"EI"|"BMC"|"BDC"|"EMC"=>{},
  "'"=>{if op.operands.len()!=1{return Err(invalid("Malformed paragraph quote."));}output.push(Operation::new("T*",vec![]));},
  "\""=>{
   if op.operands.len()!=3{return Err(invalid("Malformed paragraph quote."));}
   output.extend([Operation::new("Tw",vec![op.operands[0].clone()]),Operation::new("Tc",vec![op.operands[1].clone()]),Operation::new("T*",vec![])]);
  },
  "S"|"s"|"f"|"F"|"f*"|"B"|"B*"|"b"|"b*"=>output.push(Operation::new("n",vec![])),
  _=>output.push(op.clone()),
 }}
 Ok(output)
}

pub(super) fn leading_adjustment(show:&Operation)->EngineResult<f64>{
 if show.operator!="TJ"{return Ok(0.);}
 let [Object::Array(values)]=show.operands.as_slice()else{return Err(invalid("Malformed paragraph text array."));};
 let mut adjustment=0.;
 for value in values{
  if let Ok(bytes)=value.as_str(){if !bytes.is_empty(){break;}}
  else{adjustment+=number(value)? as f64;}
 }
 if !adjustment.is_finite(){return Err(invalid("The paragraph initial adjustment exceeds its limit."));}Ok(adjustment)
}

/// The returned start identifies only this transaction's generated 13-operator
/// fragments. It is not read from PDF metadata or any client-provided index.
pub(super) fn insert(mut original:Vec<Operation>,mut anchor:usize,fragments:Vec<Operation>,source:&RunSnapshot,leading_adjustment:f64,clip:Option<&text_context::ShowContext>)->EngineResult<(Vec<Operation>,usize)>{
 if original.len()>100_000||anchor>original.len()||fragments.is_empty(){return Err(invalid("Invalid paragraph source insertion."));}
 let mut depth=0usize;let mut needs_scope=false;
 for op in &original{match op.operator.as_str(){
  "q"=>{depth+=1;if depth>64{return Err(invalid("Paragraph graphics nesting exceeds its limit."));}},
  "Q"=>depth=depth.checked_sub(1).ok_or_else(||invalid("Unbalanced paragraph graphics state."))?,
  _=>needs_scope|=depth==0,
 }}
 if depth!=0{return Err(invalid("Unbalanced paragraph graphics state."));}
 if needs_scope{original.insert(0,Operation::new("q",vec![]));original.push(Operation::new("Q",vec![]));anchor+=1;}
 let prefix=&original[..anchor];let context=Context::read(prefix)?;
 let restore=context.in_text||!context.stack.is_empty();
 let mut replayed=if restore{replay(prefix,&context)?}else{vec![]};
 if context.in_text{replayed.push(context.cursor(source,leading_adjustment)?);}
 let size=original.len().checked_add(fragments.len()).and_then(|n|n.checked_add(replayed.len()+context.stack.len()+2))
  .filter(|n|*n<=100_000).ok_or_else(||invalid("The edited page exceeds its operation limit."))?;
 let mut output=Vec::with_capacity(size);output.extend_from_slice(prefix);
 if restore{
  if context.in_text{output.push(Operation::new("ET",vec![]));}
  if context.path_start.is_some(){output.push(Operation::new("n",vec![]));}
  for _ in &context.stack{output.push(Operation::new("Q",vec![]));}
 }
 if let Some(clip)=clip {output.extend(clip.clip_prefix()?);}
 let start=output.len();output.extend(fragments);
 if clip.is_some() {output.push(Operation::new("Q",vec![]));}
 output.extend(replayed);output.extend_from_slice(&original[anchor..]);
 if output.len()>100_000 {return Err(invalid("The edited page exceeds its operation limit."));}
 Ok((output,start))
}

#[cfg(test)]
mod tests{
 use super::*;
 #[test]
 fn malformed_or_unbounded_state_is_not_reconstructed(){
  for operations in [
   vec![Operation::new("Q",vec![])],
   vec![Operation::new("ET",vec![])],
   vec![Operation::new("BT",vec![]),Operation::new("BT",vec![])],
   vec![Operation::new("Tm",vec![1.into(),0.into(),0.into(),1.into(),0.into(),0.into()])],
   vec![Operation::new("cm",vec![Object::Real(f32::NAN),0.into(),0.into(),1.into(),0.into(),0.into()])],
   vec![Operation::new("q",vec![]);66],
  ]{assert!(Context::read(&operations).is_err());}
 }
 #[test]
 fn clipped_text_cannot_be_recreated_by_repainting_the_prefix(){
  let operations=Content::decode(b"BT /F 12 Tf 7 Tr (Clip) Tj ET").unwrap().operations;
  assert!(Context::read(&operations).is_err());
 }
}
