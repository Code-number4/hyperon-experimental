# Metta-based Logical Inference Engine for SUMO

This project is a logical inference engine capable of parsing a subset of the Suggested Upper Merged Ontology (SUMO) and performing deductions on it.

This implementation is written in Python and uses the `hyperon-experimental` library to provide the core Metta data structures and unification algorithm.

## Components

*   **Parser:** A Python function that parses SUO-KIF S-expressions and transforms variables.
*   **Knowledge Loader:** A Python function that loads KIF files into an Atomspace.
*   **Inference Core:** A Python-based forward-chaining engine.
*   **Query Interface:** A Python function to query the Atomspace.

## How to Run

To run the inference engine on the default `socrates.kif` test case, execute the following command from the `metta-sumo-inference-engine` directory:

```bash
python3 src/engine.py
```

The script will load the data, run the inference process, and print the results of a query to find out what attribute Socrates has.
