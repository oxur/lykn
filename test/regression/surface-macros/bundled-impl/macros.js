// DD-52 test fixture: registers a simple macro via macroEnv
macroEnv.set('greet', function(nameNode) {
  return array(sym('console.log'), array(sym('template'), { type: 'string', value: 'Hello, ' }, nameNode));
});
