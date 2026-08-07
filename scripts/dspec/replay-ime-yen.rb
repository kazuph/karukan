#!/usr/bin/env ruby

require "json"
require "open3"

server = ENV.fetch(
  "KARUKAN_IMSERVER",
  File.expand_path("~/Library/Input Methods/Karukan.app/Contents/MacOS/karukan-imserver"),
)

tests = [
  ["reported-input", "", "nyuuryokutyuuninanikawoyaruto"],
  ["reported-conversion", "", "henkantyuunisenntounienmaakugatotuzen"],
  ["yen-mark", "", "enmaaku"],
  ["price", "", "kakaku"],
  ["fee", "", "ryoukin"],
  ["backslash-symbol", "", "\\"],
  ["code-context", "\\", "nyuuryoku"],
  ["yen-context", "¥", "nyuuryoku"],
]

Open3.popen3(
  { "RUST_LOG" => "error" },
  [server, File.basename(server)],
) do |stdin, stdout, stderr, wait|
  request_id = 0
  request = lambda do |method, params = {}|
    request_id += 1
    stdin.puts(JSON.generate(jsonrpc: "2.0", id: request_id, method:, params:))
    stdin.flush
    JSON.parse(stdout.gets || raise("server closed stdout"))
  end

  request.call("init")
  results = tests.map do |label, context, roman|
    request.call("reset")
    request.call("set_surrounding_text", { text: context, cursor_pos: context.length })
    roman.each_byte do |byte|
      request.call("process_key", { keysym: byte, modifiers: {}, is_release: false })
    end
    response = request.call(
      "process_key",
      { keysym: 0x20, modifiers: {}, is_release: false },
    )
    actions = response.fetch("result").fetch("actions")
    candidates = actions.find { |action| action["type"] == "show_candidates" }
      &.fetch("candidates", [])
      &.map { |candidate| candidate.fetch("text") } || []
    suspicious = candidates.select { |text| text.match?(/\A[\\¥￥]/) }
    { label:, context:, roman:, candidates:, suspicious: }
  end

  request.call("reset")
  "en".each_byte do |byte|
    request.call("process_key", { keysym: byte, modifiers: {}, is_release: false })
  end
  request.call("process_key", { keysym: 0x20, modifiers: {}, is_release: false })
  before = request.call("status").dig("result", "state")
  option_digit = request.call(
    "process_key",
    {
      keysym: "2".ord,
      modifiers: { shift: false, control: false, alt: true, super: false },
      is_release: false,
    },
  )
  option_actions = option_digit.dig("result", "actions") || []
  option_after = request.call("status").dig("result", "state")

  request.call("reset")
  request.call(
    "process_key",
    { keysym: "\\".ord, modifiers: {}, is_release: false },
  )
  request.call("process_key", { keysym: 0x20, modifiers: {}, is_release: false })
  symbol_before = request.call("status").dig("result", "state")
  option_symbol_digit = request.call(
    "process_key",
    {
      keysym: "4".ord,
      modifiers: { shift: false, control: false, alt: true, super: false },
      is_release: false,
    },
  )
  option_symbol_actions = option_symbol_digit.dig("result", "actions") || []
  option_symbol_after = request.call("status").dig("result", "state")

  request.call("reset")
  "en".each_byte do |byte|
    request.call("process_key", { keysym: byte, modifiers: {}, is_release: false })
  end
  composing_before = request.call("status").dig("result", "state")
  option_return = request.call(
    "process_key",
    {
      keysym: 0xff0d,
      modifiers: { shift: false, control: false, alt: true, super: false },
      is_release: false,
    },
  )
  composing_after = request.call("status").dig("result", "state")

  unexpected_model_prefixes = results
    .reject { |result| result[:label] == "backslash-symbol" }
    .flat_map { |result| result[:suspicious].map { |text| [result[:label], text] } }
  explicit_symbols = results.find { |result| result[:label] == "backslash-symbol" }
    .fetch(:candidates)
  failures = []
  failures << "model replay exposed yen-like prefixes: #{unexpected_model_prefixes.inspect}" unless unexpected_model_prefixes.empty?
  failures << "explicit backslash conversion lost \\, ¥, or ￥" unless ["\\", "¥", "￥"].all? { |symbol| explicit_symbols.include?(symbol) }
  failures << "Option+digit mutated conversion" unless !option_digit.dig("result", "consumed") && option_actions.empty? && before == "conversion" && option_after == "conversion"
  failures << "Option+digit selected explicit yen" unless !option_symbol_digit.dig("result", "consumed") && option_symbol_actions.empty? && symbol_before == "conversion" && option_symbol_after == "conversion"
  failures << "Option+Return mutated composing input" unless !option_return.dig("result", "consumed") && (option_return.dig("result", "actions") || []).empty? && composing_before == "composing" && composing_after == "composing"

  puts JSON.pretty_generate(
    model_replay: results,
    option_digit_probe: {
      state_before: before,
      state_after: option_after,
      consumed: option_digit.dig("result", "consumed"),
      actions: option_actions,
    },
    option_symbol_digit_probe: {
      state_before: symbol_before,
      state_after: option_symbol_after,
      consumed: option_symbol_digit.dig("result", "consumed"),
      actions: option_symbol_actions,
    },
    option_return_probe: {
      state_before: composing_before,
      state_after: composing_after,
      consumed: option_return.dig("result", "consumed"),
      actions: option_return.dig("result", "actions") || [],
    },
    verification_failures: failures,
  )

  stdin.close
  stderr_output = stderr.read
  warn(stderr_output) unless stderr_output.empty?
  raise "server failed with #{wait.value.exitstatus}" unless wait.value.success?
  raise failures.join("; ") unless failures.empty?
end
