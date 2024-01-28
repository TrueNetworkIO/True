@inline
function a(): i32 {
  return i32.load(0);
}

@inline
function b(): i32 {
  return i32.load(4);
}

                     
export function calc(): i64 {
  let p1 = a();
  let p2 = b();


  return p1 + p2;
}