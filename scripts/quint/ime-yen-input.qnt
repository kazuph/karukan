module imeYenInput {
  type StateT = {
    phase: int,
    selected: int,
    modelOutputHasYenPrefix: bool,
    visibleYen: bool,
    engineBufferHasYen: bool,
    explicitYenInput: bool,
    explicitYenChoice: bool,
    modifiedKeyMutatedSelection: bool,
    liveConversionEnabled: bool,
  }

  var st: StateT

  action init = all {
    st' = {
      phase: 0,
      selected: 0,
      modelOutputHasYenPrefix: false,
      visibleYen: false,
      engineBufferHasYen: false,
      explicitYenInput: false,
      explicitYenChoice: false,
      modifiedKeyMutatedSelection: false,
      liveConversionEnabled: false,
    },
  }

  action startComposing = all {
    st.phase == 0,
    st' = {
      ...st,
      phase: 1,
      selected: 0,
      visibleYen: false,
      modifiedKeyMutatedSelection: false,
    },
  }

  action typeOrdinaryKey = all {
    st.phase == 1,
    st' = { ...st, modifiedKeyMutatedSelection: false },
  }

  action typeYenKey = all {
    st.phase == 1,
    st' = {
      ...st,
      visibleYen: true,
      engineBufferHasYen: true,
      explicitYenInput: true,
      modifiedKeyMutatedSelection: false,
    },
  }

  // backend.rs rejects model output that starts with backslash, ¥, or ￥.
  action modelReturnsYen = all {
    st.phase == 1,
    st' = {
      ...st,
      modelOutputHasYenPrefix: true,
      visibleYen: false,
      modifiedKeyMutatedSelection: false,
    },
  }

  action enterConversion = all {
    st.phase == 1,
    st' = {
      ...st,
      phase: 2,
      selected: 0,
      visibleYen: false,
      modifiedKeyMutatedSelection: false,
    },
  }

  action plainDigitSelectsExplicitYen = all {
    st.phase == 2,
    st.explicitYenInput,
    st' = {
      ...st,
      selected: 1,
      visibleYen: true,
      explicitYenChoice: true,
      modifiedKeyMutatedSelection: false,
    },
  }

  action undocumentedModifiedKeyPassesThrough = all {
    st.phase == 1 or st.phase == 2,
    st' = {
      ...st,
      modifiedKeyMutatedSelection: false,
    },
  }

  action ctrlNextCandidate = all {
    st.phase == 2,
    st.selected == 0,
    st' = {
      ...st,
      selected: 1,
      modifiedKeyMutatedSelection: false,
    },
  }

  action ctrlShiftLToggle = all {
    st' = {
      ...st,
      modifiedKeyMutatedSelection: false,
      liveConversionEnabled: not(st.liveConversionEnabled),
    },
  }

  action reset = all {
    st' = {
      ...st,
      phase: 0,
      selected: 0,
      modelOutputHasYenPrefix: false,
      visibleYen: false,
      engineBufferHasYen: false,
      explicitYenInput: false,
      explicitYenChoice: false,
      modifiedKeyMutatedSelection: false,
      liveConversionEnabled: false,
    },
  }

  action step = any {
    startComposing,
    typeOrdinaryKey,
    typeYenKey,
    modelReturnsYen,
    enterConversion,
    plainDigitSelectsExplicitYen,
    undocumentedModifiedKeyPassesThrough,
    ctrlNextCandidate,
    ctrlShiftLToggle,
    reset,
  }

  val noUnexpectedVisibleYen =
    not(st.visibleYen) or st.explicitYenInput or st.explicitYenChoice

  val modifiedKeysDoNotMutateSelection =
    not(st.modifiedKeyMutatedSelection)

  val modelOutputYenHasExplicitOrigin =
    not(st.visibleYen and st.modelOutputHasYenPrefix) or
      st.explicitYenInput or st.explicitYenChoice

  // The Rust engine buffer itself cannot synthesize yen: it only gets the
  // character through an accepted printable key. Model/client display is a
  // separate dimension and is intentionally not collapsed into this flag.
  val engineBufferYenHasExplicitOrigin =
    not(st.engineBufferHasYen) or st.explicitYenInput
}
