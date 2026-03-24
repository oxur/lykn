const greeting = "hello, world";
const greet = name => console.log(greeting + ", " + name + "!");
greet("lykn");
const taglines = ["S-expression syntax for JavaScript", "Good luck — lykn", "Closure, in every sense"];
const pick = () => {
  let idx = Math.floor(Math.random() * taglines.length);
  return taglines.at(idx);
};
console.log(pick());
