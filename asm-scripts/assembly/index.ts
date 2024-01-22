@external("host", "print")
export declare function print(i: i32): void

export function calc(): i64 {
  const a = i32.load(0);
  print(memory.size())
  print(a);
  const b = i32.load(sizeof<i32>());
  print(b);
  return a * b;
}