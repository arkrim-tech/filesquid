/**
 * Compile-time completeness check for run-time copies of contract enums.
 *
 * The generated contract types have no run-time values, so the client writes enum values out
 * to validate response bodies. Declaring such a list with {@link CompleteList} makes `tsc` fail
 * when the contract adds a value the list lacks, and `satisfies` rejects values the contract
 * does not define.
 */

/**
 * The list itself when it names every member of the generated `Union`, otherwise `never`.
 * Type-only.
 */
export type CompleteList<Union, List extends readonly Union[]> = [
  Exclude<Union, List[number]>,
] extends [never]
  ? List
  : never;
