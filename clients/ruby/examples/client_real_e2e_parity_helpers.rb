# frozen_string_literal: true

module ClientRealE2EParityHelpers
  module_function

  def document_from_corpus(corpus, source)
    document = source.dup
    fields = corpus.fetch("schema").fetch("fields")
    document.each do |name, value|
      next unless fields.dig(name, "type") == "float64" && value.is_a?(Integer)

      document[name] = value.to_f
    end
    document
  end

  def query_value(corpus, field, value)
    return value.to_f if corpus.dig("schema", "fields", field, "type") == "float64" && value.is_a?(Integer)

    value
  end
end
