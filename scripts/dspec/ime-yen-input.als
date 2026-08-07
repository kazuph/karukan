module ime_yen_input

abstract sig Phase {}
one sig Empty, Composing, Conversion extends Phase {}

abstract sig Flag {}
one sig Set, Unset extends Flag {}

abstract sig Key {}
one sig Letter, Space, Digit, Down, Up, Tab, PageUp, PageDown, ReturnKey, Escape, Backspace,
  Delete, Left, Right, Home, End, YenKey, AKey, BKey, EKey, FKey, KKey, NKey, PKey, LKey extends Key {}

abstract sig Candidate {}
one sig OrdinaryCandidate, YenCandidate extends Candidate {}

abstract sig CandidateOrigin {}
one sig Model, Dictionary, Rewriter, Fallback extends CandidateOrigin {}

abstract sig CandidatePrefix {}
one sig NoPrefix, BackslashPrefix, YenPrefix, FullwidthYenPrefix extends CandidatePrefix {}

abstract sig Effect {}
one sig PassThrough, Compose, Navigate, SelectOrdinary, SelectYen, DisplayModelYen,
  DisplayExplicitYen, EditComposition, ToggleLiveConversion extends Effect {}

one sig Decision {
  phase: one Phase,
  control: one Flag,
  option: one Flag,
  shift: one Flag,
  superKey: one Flag,
  key: one Key,
  candidate: one Candidate,
  candidateOrigin: one CandidateOrigin,
  candidatePrefix: one CandidatePrefix,
  predictionVisible: one Flag,
  explicitTop: one Candidate,
  effect: one Effect
}

pred displaysExplicitYen[d: Decision] {
  d.phase = Composing
  and d.control = Unset and d.option = Unset and d.superKey = Unset
  and d.key = YenKey and d.explicitTop = YenCandidate
}

pred navigates[d: Decision] {
  d.superKey = Unset and (
    d.phase = Composing and d.control = Set and d.option = Unset
      and d.predictionVisible = Set and d.key in NKey + PKey
    or d.phase = Conversion and (
      d.control = Set and d.option = Unset and d.key in NKey + PKey
      or d.control = Unset and d.option = Unset
        and d.key in Space + Down + Up + Tab + PageUp + PageDown
    )
  )
}

pred editsComposition[d: Decision] {
  d.superKey = Unset and d.control = Set and d.option = Unset and (
    d.phase = Empty and d.key = Space
    or d.phase = Composing and d.key in Space + AKey + BKey + EKey + FKey + KKey
  )
}

pred candidateAccepted[d: Decision] {
  not (d.candidateOrigin = Model and d.candidatePrefix != NoPrefix)
}

pred selectsYen[d: Decision] {
  d.phase = Conversion and d.control = Unset and d.option = Unset and d.superKey = Unset
    and d.key = Digit and d.candidate = YenCandidate
}

pred selectsOrdinary[d: Decision] {
  d.phase = Conversion and d.control = Unset and d.option = Unset and d.superKey = Unset
    and d.key = Digit and d.candidate = OrdinaryCandidate
}

pred passesThrough[d: Decision] {
  d.superKey = Set
  or d.superKey = Unset
    and (d.control = Set or d.option = Set)
    and not togglesLiveConversion[d]
    and not editsComposition[d]
    and not navigates[d]
}

pred composes[d: Decision] {
  not displaysExplicitYen[d]
  and d.control = Unset and d.option = Unset and d.superKey = Unset
  and (
    d.phase in Empty + Composing
      and not togglesLiveConversion[d]
    or d.phase = Conversion
      and d.key not in Space + Down + Up + Tab + PageUp + PageDown + Digit
      and not togglesLiveConversion[d]
  )
}

pred togglesLiveConversion[d: Decision] {
  d.control = Set and d.option = Unset and d.shift = Set and d.superKey = Unset and d.key = LKey
}

fact CurrentImplementation {
  all d: Decision {
    d.effect = DisplayExplicitYen iff displaysExplicitYen[d]
    d.effect = ToggleLiveConversion iff togglesLiveConversion[d]
    d.effect = Navigate iff navigates[d]
    d.effect = SelectYen iff selectsYen[d]
    d.effect = SelectOrdinary iff selectsOrdinary[d]
    d.effect = PassThrough iff passesThrough[d]
    d.effect = Compose iff composes[d]
    d.effect = EditComposition iff editsComposition[d]
  }
}

pred hasSystemModifier[d: Decision] {
  d.control = Set or d.option = Set or d.superKey = Set
}

pred isDocumentedModifiedAction[d: Decision] {
  togglesLiveConversion[d] or editsComposition[d]
  or navigates[d] and d.control = Set
}

assert ModifierIsolation {
  hasSystemModifier[Decision]
    and not isDocumentedModifiedAction[Decision]
    implies Decision.effect = PassThrough
}

assert NoUnexpectedYen {
  Decision.candidateOrigin = Model
    and Decision.candidatePrefix in BackslashPrefix + YenPrefix + FullwidthYenPrefix
    implies not candidateAccepted[Decision]
}

assert ExplicitSymbolPrefixesRemainAvailable {
  Decision.candidateOrigin in Dictionary + Rewriter
    and Decision.candidatePrefix in BackslashPrefix + YenPrefix + FullwidthYenPrefix
    implies candidateAccepted[Decision]
}

assert ExplicitYenRemainsAvailable {
  Decision.phase = Composing
    and Decision.control = Unset
    and Decision.option = Unset
    and Decision.superKey = Unset
    and Decision.key = YenKey
    and Decision.explicitTop = YenCandidate
    implies Decision.effect = DisplayExplicitYen
}

assert CtrlShiftLRemainsAvailable {
  Decision.control = Set
    and Decision.option = Unset
    and Decision.shift = Set
    and Decision.superKey = Unset
    and Decision.key = LKey
    implies Decision.effect = ToggleLiveConversion
}

pred modifierIsolationWitness {
  Decision.phase = Composing
  Decision.control = Unset
  Decision.option = Set
  Decision.superKey = Unset
  Decision.key = ReturnKey
}

pred explicitYenWitness {
  Decision.phase = Composing
  Decision.control = Unset
  Decision.option = Unset
  Decision.superKey = Unset
  Decision.key = YenKey
  Decision.candidateOrigin = Rewriter
  Decision.candidatePrefix = BackslashPrefix
  Decision.explicitTop = YenCandidate
}

pred modelPrefixWitness {
  Decision.phase = Empty
  Decision.control = Unset
  Decision.option = Unset
  Decision.superKey = Unset
  Decision.candidateOrigin = Model
  Decision.candidatePrefix = YenPrefix
}

pred dictionaryPrefixWitness {
  Decision.phase = Empty
  Decision.control = Unset
  Decision.option = Unset
  Decision.superKey = Unset
  Decision.candidateOrigin = Dictionary
  Decision.candidatePrefix = FullwidthYenPrefix
}

pred ctrlNWitness {
  Decision.phase = Conversion
  Decision.control = Set
  Decision.option = Unset
  Decision.superKey = Unset
  Decision.key = NKey
}

pred ctrlShiftLWitness {
  Decision.control = Set
  Decision.option = Unset
  Decision.shift = Set
  Decision.superKey = Unset
  Decision.key = LKey
}

pred superConversionWitness {
  Decision.phase = Conversion
  Decision.superKey = Set
  Decision.key = Space
}

check ModifierIsolation for exactly 1 Decision
check NoUnexpectedYen for exactly 1 Decision
check ExplicitSymbolPrefixesRemainAvailable for exactly 1 Decision
check ExplicitYenRemainsAvailable for exactly 1 Decision
check CtrlShiftLRemainsAvailable for exactly 1 Decision
run modifierIsolationWitness for exactly 1 Decision
run explicitYenWitness for exactly 1 Decision
run modelPrefixWitness for exactly 1 Decision
run dictionaryPrefixWitness for exactly 1 Decision
run ctrlNWitness for exactly 1 Decision
run ctrlShiftLWitness for exactly 1 Decision
run superConversionWitness for exactly 1 Decision
