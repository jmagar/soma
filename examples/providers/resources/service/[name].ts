export async function read(input) {
  return { text: `status for ${input.params.name}: healthy` };
}
