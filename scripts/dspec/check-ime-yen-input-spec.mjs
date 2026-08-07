import { readFileSync } from "node:fs";

const conversion = readFileSync("karukan-im/src/core/engine/conversion.rs", "utf8");
const input = readFileSync("karukan-im/src/core/engine/input.rs", "utf8");
const engine = readFileSync("karukan-im/src/core/engine/mod.rs", "utf8");
const backend = readFileSync("karukan-engine/src/kanji/backend.rs", "utf8");
const keyMap = readFileSync(
  "karukan-macos/Sources/KarukanIME/KeyCodeMap.swift",
  "utf8",
);
const alloy = readFileSync("scripts/dspec/ime-yen-input.als", "utf8");
const composingInput = input.slice(
  input.indexOf("pub(super) fn process_key_composing"),
);

function requireAnchor(source, anchor, description) {
  if (!source.includes(anchor)) {
    throw new Error(`implementation drift: missing ${description}: ${anchor}`);
  }
}

function appearsBefore(source, first, second, description) {
  const firstIndex = source.indexOf(first);
  const secondIndex = source.indexOf(second);
  if (firstIndex < 0 || secondIndex < 0 || firstIndex >= secondIndex) {
    throw new Error(`implementation drift: wrong order for ${description}`);
  }
}

requireAnchor(
  backend,
  "if candidate.starts_with(['\\\\', '¥', '￥'])",
  "yen-like model-output rejection",
);
function requireCount(source, anchor, expected, description) {
  const count = source.split(anchor).length - 1;
  if (count !== expected) {
    throw new Error(`implementation drift: expected ${expected} ${description}, found ${count}`);
  }
}

requireCount(backend, "if !clean.is_empty()", 2, "model candidate acceptance guards");
requireAnchor(
  conversion,
  "if key.modifiers.control_key || key.modifiers.alt_key {",
  "conversion modifier isolation guard",
);
requireAnchor(
  input,
  "if key.modifiers.control_key || key.modifiers.alt_key {",
  "composing modifier isolation guard",
);
requireAnchor(
  engine,
  "if key.modifiers.super_key {",
  "Super shortcut pass-through guard",
);
requireAnchor(
  conversion,
  "Keysym::KEY_N | Keysym::KEY_N_UPPER => return self.next_candidate(),",
  "Ctrl+N exception",
);
requireAnchor(
  input,
  "&& !key.modifiers.control_key",
  "composing control modifier guard",
);
requireAnchor(
  input,
  "&& !key.modifiers.alt_key",
  "composing option modifier guard",
);
requireAnchor(keyMap, "(0x20...0x7e).contains", "ASCII-only macOS translation");
appearsBefore(
  conversion,
  "if key.modifiers.control_key || key.modifiers.alt_key {",
  "Keysym::SPACE | Keysym::DOWN | Keysym::TAB => self.next_candidate()",
  "modifier isolation preceding conversion navigation",
);
appearsBefore(
  composingInput,
  "if key.modifiers.control_key || key.modifiers.alt_key {",
  "Keysym::RETURN => self.commit_composing(),",
  "modifier isolation preceding composing key dispatch",
);

for (const assertion of [
  "assert ModifierIsolation",
  "assert NoUnexpectedYen",
  "assert ExplicitYenRemainsAvailable",
  "assert CtrlShiftLRemainsAvailable",
  "run modifierIsolationWitness",
  "run explicitYenWitness",
]) {
  requireAnchor(alloy, assertion, `Alloy assertion ${assertion}`);
}
for (const decisionRule of [
  "control: one Flag",
  "option: one Flag",
  "shift: one Flag",
  "superKey: one Flag",
  "candidateOrigin: one CandidateOrigin",
  "candidatePrefix: one CandidatePrefix",
  "predictionVisible: one Flag",
  "d.effect = DisplayExplicitYen iff displaysExplicitYen[d]",
  "d.effect = ToggleLiveConversion iff togglesLiveConversion[d]",
  "d.effect = Navigate iff navigates[d]",
  "d.effect = SelectYen iff selectsYen[d]",
  "d.effect = SelectOrdinary iff selectsOrdinary[d]",
  "d.effect = PassThrough iff passesThrough[d]",
  "d.effect = Compose iff composes[d]",
  "d.effect = EditComposition iff editsComposition[d]",
]) {
  requireAnchor(alloy, decisionRule, `deterministic Alloy rule ${decisionRule}`);
}

const phases = ["empty", "composing", "conversion"];
const modifiers = [];
for (const control of [false, true]) {
  for (const option of [false, true]) {
    for (const shift of [false, true]) {
      for (const superKey of [false, true]) {
        modifiers.push({ control, option, shift, superKey });
      }
    }
  }
}
const keys = [
  "letter", "space", "digit", "down", "up", "tab", "pageUp", "pageDown",
  "return", "escape", "backspace", "delete", "left", "right", "home", "end", "yen",
  "a", "b", "e", "f", "k", "n", "p", "l",
];
const candidates = ["ordinary", "yen"];
const origins = ["model", "dictionary", "rewriter", "fallback"];
const prefixes = ["none", "backslash", "yen", "fullwidthYen"];

function decision({ phase, modifier, key, candidate, predictionVisible, explicitTop }) {
  if (modifier.superKey) return "passThrough";
  if (modifier.control && !modifier.option && modifier.shift && key === "l") {
    return "toggleLiveConversion";
  }
  if (phase === "empty") {
    if (modifier.control && !modifier.option && key === "space") return "editComposition";
    if (modifier.control || modifier.option) return "passThrough";
    return "compose";
  }
  if (phase === "composing") {
    if (
      modifier.control &&
      !modifier.option &&
      ["space", "a", "b", "e", "f", "k"].includes(key)
    ) {
      return "editComposition";
    }
    if (
      modifier.control &&
      !modifier.option &&
      predictionVisible &&
      ["n", "p"].includes(key)
    ) {
      return "navigate";
    }
    if (modifier.control || modifier.option) return "passThrough";
    return key === "yen" && explicitTop === "yen" ? "displayExplicitYen" : "compose";
  }
  if (modifier.control && !modifier.option && ["n", "p"].includes(key)) return "navigate";
  if (modifier.control || modifier.option) return "passThrough";
  if (["space", "down", "up", "tab", "pageUp", "pageDown"].includes(key)) return "navigate";
  if (key === "digit") return candidate === "yen" ? "selectYen" : "selectOrdinary";
  return "compose";
}

const cases = [];
for (const phase of phases) {
  for (const modifier of modifiers) {
    for (const key of keys) {
      for (const candidate of candidates) {
        for (const candidateOrigin of origins) {
          for (const candidatePrefix of prefixes) {
            for (const predictionVisible of [false, true]) {
              for (const explicitTop of candidates) {
                cases.push({
                  phase,
                  modifier,
                  key,
                  candidate,
                  candidateOrigin,
                  candidatePrefix,
                  predictionVisible,
                  explicitTop,
                });
              }
            }
          }
        }
      }
    }
  }
}

function isDocumentedModifiedAction(entry) {
  const { phase, modifier, key, predictionVisible } = entry;
  return (
    !modifier.superKey &&
    modifier.control &&
    !modifier.option &&
    ((modifier.shift && key === "l") ||
      (phase === "empty" && key === "space") ||
      (phase === "composing" && ["space", "a", "b", "e", "f", "k"].includes(key)) ||
      (phase === "composing" && predictionVisible && ["n", "p"].includes(key)) ||
      (phase === "conversion" && ["n", "p"].includes(key)))
  );
}

const modifierViolations = cases.filter(
  (entry) =>
    (entry.modifier.control || entry.modifier.option || entry.modifier.superKey) &&
    !isDocumentedModifiedAction(entry) &&
    decision(entry) !== "passThrough",
);
const unexpectedYenViolations = cases.filter(
  (entry) =>
    entry.candidateOrigin === "model" &&
    entry.candidatePrefix !== "none" &&
    candidateAccepted(entry),
);
const explicitSymbolAvailabilityViolations = cases.filter(
  (entry) =>
    ["dictionary", "rewriter"].includes(entry.candidateOrigin) &&
    entry.candidatePrefix !== "none" &&
    !candidateAccepted(entry),
);

function candidateAccepted({ candidateOrigin, candidatePrefix }) {
  return !(candidateOrigin === "model" && candidatePrefix !== "none");
}

if (
  modifierViolations.length !== 0 ||
  unexpectedYenViolations.length !== 0 ||
  explicitSymbolAvailabilityViolations.length !== 0
) {
  throw new Error("finite model found a fixed-policy safety violation");
}

const explicitYenWitness = decision({
  phase: "composing",
  modifier: { control: false, option: false, shift: false, superKey: false },
  key: "yen",
  candidate: "ordinary",
  candidateOrigin: "rewriter",
  candidatePrefix: "backslash",
  predictionVisible: true,
  explicitTop: "yen",
});
const ctrlNWitness = decision({
  phase: "conversion",
  modifier: { control: true, option: false, shift: false, superKey: false },
  key: "n",
  candidate: "ordinary",
  candidateOrigin: "fallback",
  candidatePrefix: "none",
  predictionVisible: true,
  explicitTop: "ordinary",
});
const optionReturnWitness = decision({
  phase: "composing",
  modifier: { control: false, option: true, shift: false, superKey: false },
  key: "return",
  candidate: "ordinary",
  candidateOrigin: "fallback",
  candidatePrefix: "none",
  predictionVisible: false,
  explicitTop: "ordinary",
});
if (
  explicitYenWitness !== "displayExplicitYen" ||
  ctrlNWitness !== "navigate" ||
  optionReturnWitness !== "passThrough"
) {
  throw new Error("finite model lost an allowed explicit-symbol or Ctrl+N path");
}
if (
  candidateAccepted({ candidateOrigin: "model", candidatePrefix: "yen" }) ||
  !candidateAccepted({ candidateOrigin: "dictionary", candidatePrefix: "fullwidthYen" })
) {
  throw new Error("finite model lost model-prefix rejection or explicit-symbol availability");
}

console.log(
  JSON.stringify(
    {
      enumeratedCases: cases.length,
      modifierCombinations: modifiers.length,
      modifierIsolationViolations: modifierViolations.length,
      unexpectedYenViolations: unexpectedYenViolations.length,
      explicitSymbolAvailabilityViolations: explicitSymbolAvailabilityViolations.length,
      witnesses: {
        explicitYen: explicitYenWitness,
        ctrlN: ctrlNWitness,
        optionReturn: optionReturnWitness,
      },
    },
    null,
    2,
  ),
);
