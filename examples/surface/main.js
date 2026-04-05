const greeting = "hello, world";
function greet(name) {
  if (typeof name !== "string") throw new TypeError("greet: arg 'name' expected string, got " + typeof name);
  console.log(greeting + ", " + name + "!");
}
greet("lykn");
const taglines = ["S-expression syntax for JavaScript", "Good luck — lykn", "Closure, in every sense"];
function pick() {
  const idx = Math.floor(Math.random() * taglines.length);
  return taglines.at(idx);
}
console.log(pick());

