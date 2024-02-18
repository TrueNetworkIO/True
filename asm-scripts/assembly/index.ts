@inline
function load_lines_of_code(): u32 {
  return load<u32>(0);
}

@inline
function load_evaluated_years_of_experience(): u32 {
  return load<u32>(4);
}

@inline
function load_number_of_prs(): u32 {
  return load<u32>(8);
}
                     
export function calc(): i64 {
  let loc = load_lines_of_code();
  let prs = load_number_of_prs();
  let yoe = load_evaluated_years_of_experience();

  return yoe * prs + loc;
}