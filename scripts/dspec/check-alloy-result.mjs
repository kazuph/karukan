import { readFileSync } from "node:fs";

const receiptPath = process.argv[2];
if (!receiptPath) {
  throw new Error("usage: node check-alloy-result.mjs <receipt.json>");
}

const receipt = JSON.parse(readFileSync(receiptPath, "utf8"));

function decisionFor(commandName) {
  const command = receipt.commands?.[commandName];
  const decision = command?.solution?.[0]?.instances?.[0]?.values?.["Decision$0"];
  if (!decision) {
    throw new Error(`${commandName}: expected a SAT counterexample`);
  }
  return Object.fromEntries(
    Object.entries(decision).map(([field, tuples]) => [
      field,
      tuples?.[0]?.[0]?.replace(/\$0$/, ""),
    ]),
  );
}

function requireNoCounterexample(commandName) {
  const command = receipt.commands?.[commandName];
  if (!command) {
    throw new Error(`${commandName}: missing Alloy check result`);
  }
  if ((command.solution ?? []).length !== 0) {
    throw new Error(`${commandName}: found a counterexample`);
  }
}

function requireFields(commandName, actual, expected) {
  for (const [field, value] of Object.entries(expected)) {
    if (actual[field] !== value) {
      throw new Error(
        `${commandName}: expected ${field}=${value}, got ${actual[field]}`,
      );
    }
  }
}

for (const commandName of [
  "ModifierIsolation",
  "NoUnexpectedYen",
  "ExplicitSymbolPrefixesRemainAvailable",
  "ExplicitYenRemainsAvailable",
  "CtrlShiftLRemainsAvailable",
]) {
  requireNoCounterexample(commandName);
}

const modifierIsolationWitness = decisionFor("modifierIsolationWitness");
requireFields("modifierIsolationWitness", modifierIsolationWitness, {
  phase: "Composing",
  control: "Unset",
  option: "Set",
  superKey: "Unset",
  key: "ReturnKey",
  effect: "PassThrough",
});

const explicitYenWitness = decisionFor("explicitYenWitness");
requireFields("explicitYenWitness", explicitYenWitness, {
  phase: "Composing",
  key: "YenKey",
  explicitTop: "YenCandidate",
  candidateOrigin: "Rewriter",
  candidatePrefix: "BackslashPrefix",
  effect: "DisplayExplicitYen",
});

const modelPrefixWitness = decisionFor("modelPrefixWitness");
requireFields("modelPrefixWitness", modelPrefixWitness, {
  candidateOrigin: "Model",
  candidatePrefix: "YenPrefix",
});

const dictionaryPrefixWitness = decisionFor("dictionaryPrefixWitness");
requireFields("dictionaryPrefixWitness", dictionaryPrefixWitness, {
  candidateOrigin: "Dictionary",
  candidatePrefix: "FullwidthYenPrefix",
});

const ctrlNWitness = decisionFor("ctrlNWitness");
requireFields("ctrlNWitness", ctrlNWitness, {
  phase: "Conversion",
  control: "Set",
  option: "Unset",
  key: "NKey",
  effect: "Navigate",
});

const ctrlShiftLWitness = decisionFor("ctrlShiftLWitness");
requireFields("ctrlShiftLWitness", ctrlShiftLWitness, {
  control: "Set",
  shift: "Set",
  key: "LKey",
  effect: "ToggleLiveConversion",
});

const superConversionWitness = decisionFor("superConversionWitness");
requireFields("superConversionWitness", superConversionWitness, {
  phase: "Conversion",
  superKey: "Set",
  key: "Space",
  effect: "PassThrough",
});

console.log(
  JSON.stringify(
    {
      alloyVersion: "6.2.0",
      solver: receipt.solver,
      checks: "no counterexamples",
      witnesses: {
        modifierIsolation: modifierIsolationWitness,
        explicitYen: explicitYenWitness,
        modelPrefix: modelPrefixWitness,
        dictionaryPrefix: dictionaryPrefixWitness,
        ctrlN: ctrlNWitness,
        ctrlShiftL: ctrlShiftLWitness,
        superConversion: superConversionWitness,
      },
    },
    null,
    2,
  ),
);
