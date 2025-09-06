import os
from dataclasses import dataclass
from typing import List, Any, Union, Tuple

# == Core Data Structures ==

@dataclass(frozen=True)
class Symbol:
    name: str
    def __repr__(self): return self.name

@dataclass(frozen=True)
class Variable:
    name: str
    def __repr__(self): return self.name

@dataclass(frozen=True)
class Expression:
    children: Tuple[Union['Expression', Symbol, Variable], ...]
    def __repr__(self): return f"({' '.join(map(repr, self.children))})"

Atom = Union[Expression, Symbol, Variable]

# == Unification and Substitution ==

def substitute(atom: Atom, bindings: dict) -> Atom:
    if isinstance(atom, Variable) and atom.name in bindings:
        return bindings[atom.name]
    elif isinstance(atom, Expression):
        return Expression(tuple(substitute(child, bindings) for child in atom.children))
    else:
        return atom

def match(pattern: Atom, fact: Atom) -> Union[dict, None]:
    return _match_recursive(pattern, fact, {})

def _match_recursive(pattern: Atom, fact: Atom, bindings: dict) -> Union[dict, None]:
    if isinstance(pattern, Variable):
        if pattern.name in bindings:
            return bindings if bindings[pattern.name] == fact else None
        else:
            bindings[pattern.name] = fact
            return bindings
    if isinstance(pattern, Expression) and isinstance(fact, Expression):
        if len(pattern.children) != len(fact.children): return None
        for p_child, f_child in zip(pattern.children, fact.children):
            bindings = _match_recursive(p_child, f_child, bindings)
            if bindings is None: return None
        return bindings
    return bindings if pattern == fact else None

# == S-expression Parser ==

class SExprParser:
    def __init__(self, text):
        clean_text = "".join(line.split(';', 1)[0] for line in text.splitlines())
        self.tokens = clean_text.replace('(', ' ( ').replace(')', ' ) ').split()
        self.index = 0
    def _parse_rec(self):
        if self.index >= len(self.tokens): raise SyntaxError("Unexpected end of input")
        token = self.tokens[self.index]; self.index += 1
        if token == '(':
            expr = []
            while self.tokens[self.index] != ')': expr.append(self._parse_rec())
            self.index += 1
            return tuple(expr)
        else: return token
    def parse(self):
        expressions = []
        while self.index < len(self.tokens): expressions.append(self._parse_rec())
        return expressions

# == Main Inference Engine ==

class InferenceEngine:
    def __init__(self):
        self.atomspace = set()

    def _sexpr_to_atom(self, sexpr: Any) -> Atom:
        if isinstance(sexpr, tuple):
            return Expression(tuple(self._sexpr_to_atom(child) for child in sexpr))
        elif isinstance(sexpr, str):
            return Variable(sexpr) if sexpr.startswith('?') else Symbol(sexpr)
        raise TypeError(f"Unexpected S-expression type: {type(sexpr)}")

    def load_kif_file(self, filepath: str):
        print(f"--- Loading knowledge from {filepath} ---")
        try:
            with open(filepath, "r") as f: content = f.read()
            expressions = SExprParser(content).parse()
            for sexpr in expressions:
                metta_atom = self._sexpr_to_atom(sexpr)
                print(f"  Adding: {metta_atom}")
                self.atomspace.add(metta_atom)
        except Exception as e: print(f"ERROR: Failed to load {filepath}: {e}")

    def forward_chain(self):
        print("\n--- Running Inference Engine ---")
        while True:
            new_facts = set()
            rules = [atom for atom in self.atomspace if isinstance(atom, Expression) and atom.children and atom.children[0] == Symbol('=>')]

            for rule in rules:
                antecedent = rule.children[1]
                consequent = rule.children[2]
                for fact in self.atomspace:
                    bindings = match(antecedent, fact)
                    if bindings is not None:
                        new_fact = substitute(consequent, bindings)
                        if new_fact not in self.atomspace and new_fact not in new_facts:
                            print(f"  Inferred: {new_fact}")
                            new_facts.add(new_fact)

            if not new_facts:
                print("  Inference complete (fixpoint reached).")
                break
            else:
                self.atomspace.update(new_facts)

    def query(self, query_string: str) -> List[Atom]:
        print(f"\n--- Querying for: {query_string} ---")
        query_atom = self._sexpr_to_atom(SExprParser(query_string).parse()[0])
        results = []
        for fact in self.atomspace:
            bindings = match(query_atom, fact)
            if bindings is not None:
                results.append(substitute(query_atom, bindings))
        return results

if __name__ == "__main__":
    if os.path.basename(os.getcwd()) != 'metta-sumo-inference-engine':
        os.chdir('metta-sumo-inference-engine')

    engine = InferenceEngine()
    engine.load_kif_file("data/socrates.kif")
    engine.forward_chain()
    query_results = engine.query("(attribute Socrates ?what)")

    print("\nQuery results:")
    if not query_results:
        print("  No results found.")
    else:
        for result in query_results:
            print(f"  {result}")
