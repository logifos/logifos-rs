# frozen_string_literal: true

# Renders MathML files to SVG for the canon comparison. Reads a plan file of
# `source<TAB>target` lines; every line renders independently and a render
# failure writes `<target>.error` with the message instead of the SVG.
require "lasem"

rendered = 0
errored = 0
File.readlines(ARGV.fetch(0), chomp: true).each do |line|
  next if line.strip.empty?

  source, target = line.split("\t", 2)
  begin
    svg = Lasem::Renderer.render(File.read(source))
    raise "render returned no SVG" if svg.nil? || svg.empty?

    File.binwrite(target, svg)
    rendered += 1
  rescue StandardError => failure
    errored += 1
    File.write("#{target}.error", "#{failure.class}: #{failure.message}\n")
  end
end
puts "rendered #{rendered}, errored #{errored}"
