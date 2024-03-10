@inline
function load_no_of_repos(): u32 {
  return load<u32>(0);
}

@inline
function load_no_of_private_repos(): u64 {
  return load<u64>(4);
}

export function calc(): i64 {
  let loc = load_no_of_repos();
  let yoe = load_no_of_private_repos();

  return yoe + loc;
}