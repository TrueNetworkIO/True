@inline
function load_followers(): i32 {
  return i32.load(0);
}

@inline
function load_stars(): i32 {
  return i32.load(4);
}

@external("host", "print")
export declare function print(p: i32): void;
                     
export function calc(): i64 {
  let followers = load_followers();
  let stars = load_stars();

  print(followers);
  print(stars);

  return followers + stars;
}