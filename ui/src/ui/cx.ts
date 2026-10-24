/** Ghép tên class, bỏ qua giá trị rỗng (class của CSS Modules có thể là `undefined`). */
export function cx(...names: Array<string | false | null | undefined>): string {
  return names.filter(Boolean).join(" ");
}
