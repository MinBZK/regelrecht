Feature: Note resolution (RFC-005, RFC-016)
  A note anchors to legal text via a W3C TextQuoteSelector: an exact quote
  plus optional prefix/suffix context. The selector is content-addressed, so
  a note resolves on any law version where the text exists, surviving article
  renumbering and minor textual changes (via fuzzy matching).

  Scenario: Exact match
    Given a law with the following articles:
      | number | text                                                                         |
      | 2      | heeft de verzekerde aanspraak op een zorgtoeslag ter grootte van dat verschil |
    And a note selecting "zorgtoeslag" with prefix "op een " and suffix " ter grootte"
    When the note is resolved
    Then the note resolves to article "2"
    And the note is an exact match

  Scenario: Article renumbered keeps the note anchored to the text
    # A new article 1a is inserted; the annotated text moves to article 4a.
    # The content-addressed selector still finds it.
    Given a law with the following articles:
      | number | text                                                                         |
      | 1a     | Een nieuw ingevoegd artikel zonder relevante inhoud.                          |
      | 4a     | heeft de verzekerde aanspraak op een zorgtoeslag ter grootte van dat verschil |
    And a note selecting "zorgtoeslag" with prefix "op een " and suffix " ter grootte"
    When the note is resolved
    Then the note resolves to article "4a"

  Scenario: Minor text change still resolves via fuzzy matching
    # Staatsblad 2008, 516 changed wording; the note survives as a fuzzy match.
    Given a law with the following articles:
      | number | text                                                                       |
      | 2      | heeft de verzekerde recht op een zorgtoeslag ter grootte van het verschil  |
    And a note selecting "aanspraak op een zorgtoeslag" with prefix "heeft de verzekerde " and suffix " ter grootte van dat verschil"
    When the note is resolved
    Then the note resolves to article "2"
    And the note is a fuzzy match

  Scenario: Text removed orphans the note
    Given a law with the following articles:
      | number | text                                          |
      | 2      | Geheel andere tekst zonder de gezochte zin.   |
    And a note selecting "zorgtoeslag ter grootte van dat verschil" with prefix "aanspraak op een " and suffix ""
    When the note is resolved
    Then the note is orphaned

  Scenario: Common word without context is ambiguous
    Given a law with the following articles:
      | number | text                                                  |
      | 2      | de verzekerde en de verzekerde en nog een verzekerde  |
    And a note selecting "verzekerde"
    When the note is resolved
    Then the note is ambiguous

  Scenario: Context disambiguates a common word
    Given a law with the following articles:
      | number | text                                                                                       |
      | 2      | de verzekerde betaalt; de verzekerde ontvangt; heeft de verzekerde aanspraak op zorgtoeslag |
    And a note selecting "verzekerde" with prefix "heeft de " and suffix " aanspraak"
    When the note is resolved
    Then the note resolves to article "2"

  Scenario: A correct hint finds the match
    Given a law with the following articles:
      | number | text                                                  |
      | 1      | Onbelangrijke inleidende tekst.                       |
      | 2      | heeft de verzekerde aanspraak op een zorgtoeslag hier |
    And a note selecting "zorgtoeslag" with prefix "op een " and suffix " hier"
    And the note hints article "2"
    When the note is resolved
    Then the note resolves to article "2"

  Scenario: An outdated hint falls back to a full search
    # The hint points at article 9, which no longer contains the text.
    # Resolution must still find it in article 2.
    Given a law with the following articles:
      | number | text                                                  |
      | 2      | heeft de verzekerde aanspraak op een zorgtoeslag hier |
      | 9      | Niets relevants in dit artikel.                       |
    And a note selecting "zorgtoeslag" with prefix "op een " and suffix " hier"
    And the note hints article "9" at position 0 to 5
    When the note is resolved
    Then the note resolves to article "2"

  # === Ambiguity tracking (RFC-016 Decision 6) ===
  # A questioning note over an open norm still has to resolve to the text it
  # is about; the ambiguity state lives in a tagging body, the resolver only
  # cares about anchoring.

  Scenario: A questioning note over an open norm resolves to the delegating text
    Given a law with the following articles:
      | number | text                                                                              |
      | 2      | Bij ministeriële regeling worden nadere regels gesteld over de hardheidsclausule. |
    And a note selecting "bij ministeriële regeling" with prefix "" and suffix " worden nadere regels"
    When the note is resolved
    Then the note resolves to article "2"

  Scenario: A note about a missing document anchors to the text that requires it
    Given a law with the following articles:
      | number | text                                                                          |
      | 5      | De Belastingdienst handelt overeenkomstig de beleidsregels van de Belastingdienst omtrent hardheid. |
    And a note selecting "beleidsregels van de Belastingdienst" with prefix "overeenkomstig de " and suffix " omtrent"
    When the note is resolved
    Then the note resolves to article "5"
