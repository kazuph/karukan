inductive Phase where
  | empty
  | composing
  | conversion
  deriving BEq, DecidableEq, Repr

structure ModifierState where
  control : Bool
  option : Bool
  shift : Bool
  super : Bool
  deriving BEq, DecidableEq, Repr

inductive Key where
  | letter
  | space
  | digit
  | down
  | up
  | tab
  | pageUp
  | pageDown
  | returnKey
  | escape
  | backspace
  | delete
  | left
  | right
  | home
  | endKey
  | yen
  | a
  | b
  | e
  | f
  | k
  | n
  | p
  | l
  deriving BEq, DecidableEq, Repr

inductive Candidate where
  | ordinary
  | yen
  deriving BEq, DecidableEq, Repr

inductive CandidateOrigin where
  | model
  | dictionary
  | rewriter
  | fallback
  deriving BEq, DecidableEq, Repr

inductive CandidatePrefix where
  | none
  | backslash
  | yen
  | fullwidthYen
  deriving BEq, DecidableEq, Repr

inductive Effect where
  | passThrough
  | compose
  | navigate
  | selectOrdinary
  | selectYen
  | displayModelYen
  | displayExplicitYen
  | editComposition
  | toggleLiveConversion
  deriving BEq, DecidableEq, Repr

structure InputCase where
  phase : Phase
  modifier : ModifierState
  key : Key
  candidate : Candidate
  candidateOrigin : CandidateOrigin
  candidatePrefix : CandidatePrefix
  predictionVisible : Bool
  explicitTop : Candidate
  deriving BEq, DecidableEq, Repr

def currentDecision (c : InputCase) : Effect :=
  if c.modifier.super then .passThrough
  else if c.modifier.control && !c.modifier.option && c.modifier.shift && c.key == .l then
    .toggleLiveConversion
  else match c.phase with
    | .empty =>
        if c.modifier.control && !c.modifier.option && c.key == .space then .editComposition
        else if c.modifier.control || c.modifier.option then .passThrough
        else .compose
    | .composing =>
        if c.modifier.control && !c.modifier.option &&
            (c.key == .space || c.key == .a || c.key == .b || c.key == .e ||
              c.key == .f || c.key == .k) then .editComposition
        else if c.modifier.control && !c.modifier.option && c.predictionVisible &&
            (c.key == .n || c.key == .p) then .navigate
        else if c.modifier.control || c.modifier.option then .passThrough
        else if c.key == .yen && c.explicitTop == .yen then .displayExplicitYen
        else .compose
    | .conversion =>
        if c.modifier.control && !c.modifier.option && (c.key == .n || c.key == .p) then .navigate
        else if c.modifier.control || c.modifier.option then .passThrough
        else match c.key with
          | .space | .down | .tab | .up | .pageUp | .pageDown => .navigate
          | .digit => if c.candidate == .yen then .selectYen else .selectOrdinary
          | _ => .compose

def candidateAccepted (c : InputCase) : Bool :=
  !(c.candidateOrigin == .model && c.candidatePrefix != .none)

def hasSystemModifier (m : ModifierState) : Bool :=
  m.control || m.option || m.super

def isDocumentedModifiedAction (c : InputCase) : Bool :=
  !c.modifier.super && c.modifier.control && !c.modifier.option &&
    ((c.modifier.shift && c.key == .l) ||
      (c.phase == .empty && c.key == .space) ||
      (c.phase == .composing && (c.key == .space || c.key == .a || c.key == .b ||
        c.key == .e || c.key == .f || c.key == .k)) ||
      (c.phase == .composing && c.predictionVisible && (c.key == .n || c.key == .p)) ||
      (c.phase == .conversion && (c.key == .n || c.key == .p)))

def violatesModifierIsolation (c : InputCase) : Bool :=
  hasSystemModifier c.modifier &&
    !isDocumentedModifiedAction c &&
    currentDecision c != .passThrough

def violatesUnexpectedYen (c : InputCase) : Bool :=
  c.candidateOrigin == .model &&
    c.candidatePrefix != .none &&
    candidateAccepted c

def violatesExplicitSymbolAvailability (c : InputCase) : Bool :=
  (c.candidateOrigin == .dictionary || c.candidateOrigin == .rewriter) &&
    c.candidatePrefix != .none &&
    !candidateAccepted c

def phases := [Phase.empty, .composing, .conversion]
def booleans := [false, true]
def modifiers := booleans.flatMap fun control =>
  booleans.flatMap fun option =>
    booleans.flatMap fun shift =>
      booleans.map fun super => ModifierState.mk control option shift super
def keys := [
  Key.letter, .space, .digit, .down, .up, .tab, .pageUp, .pageDown,
  .returnKey, .escape, .backspace, .delete, .left, .right, .home, .endKey, .yen,
  .a, .b, .e, .f, .k, .n, .p, .l
]
def candidates := [Candidate.ordinary, .yen]
def origins := [CandidateOrigin.model, .dictionary, .rewriter, .fallback]
def prefixes := [CandidatePrefix.none, .backslash, .yen, .fullwidthYen]

def allCases : List InputCase :=
  phases.flatMap fun phase =>
    modifiers.flatMap fun modifier =>
      keys.flatMap fun key =>
        candidates.flatMap fun candidate =>
          origins.flatMap fun candidateOrigin =>
            prefixes.flatMap fun candidatePrefix =>
              booleans.flatMap fun predictionVisible =>
                candidates.map fun explicitTop =>
                  { phase, modifier, key, candidate, candidateOrigin, candidatePrefix,
                    predictionVisible, explicitTop }

def modifierIsolationViolations := allCases.filter violatesModifierIsolation
def unexpectedYenViolations := allCases.filter violatesUnexpectedYen
def explicitSymbolAvailabilityViolations := allCases.filter violatesExplicitSymbolAvailability

theorem explicitYenInputCanDisplayYen :
    currentDecision {
      phase := .composing,
      modifier := { control := false, option := false, shift := false, super := false },
      key := .yen,
      candidate := .yen,
      candidateOrigin := .rewriter,
      candidatePrefix := .backslash,
      predictionVisible := true,
      explicitTop := .yen
    } = .displayExplicitYen := by
  native_decide

theorem modelYenPrefixIsRejected :
    candidateAccepted {
      phase := .composing,
      modifier := { control := false, option := false, shift := false, super := false },
      key := .letter,
      candidate := .ordinary,
      candidateOrigin := .model,
      candidatePrefix := .yen,
      predictionVisible := false,
      explicitTop := .ordinary
    } = false := by
  native_decide

theorem dictionaryYenPrefixRemainsAccepted :
    candidateAccepted {
      phase := .composing,
      modifier := { control := false, option := false, shift := false, super := false },
      key := .yen,
      candidate := .yen,
      candidateOrigin := .dictionary,
      candidatePrefix := .fullwidthYen,
      predictionVisible := true,
      explicitTop := .yen
    } = true := by
  native_decide

theorem ctrlNStillNavigates :
    currentDecision {
      phase := .conversion,
      modifier := { control := true, option := false, shift := false, super := false },
      key := .n,
      candidate := .ordinary,
      candidateOrigin := .fallback,
      candidatePrefix := .none,
      predictionVisible := true,
      explicitTop := .ordinary
    } = .navigate := by
  native_decide

theorem ctrlShiftLStillTogglesLiveConversion :
    currentDecision {
      phase := .conversion,
      modifier := { control := true, option := false, shift := true, super := false },
      key := .l,
      candidate := .ordinary,
      candidateOrigin := .fallback,
      candidatePrefix := .none,
      predictionVisible := false,
      explicitTop := .ordinary
    } = .toggleLiveConversion := by
  native_decide

theorem modifierIsolationHasNoViolations :
    modifierIsolationViolations = [] := by
  native_decide

theorem unexpectedYenHasNoViolations :
    unexpectedYenViolations = [] := by
  native_decide

theorem explicitSymbolAvailabilityHasNoViolations :
    explicitSymbolAvailabilityViolations = [] := by
  native_decide

#eval modifierIsolationViolations.length
#eval unexpectedYenViolations.length
#eval explicitSymbolAvailabilityViolations.length
