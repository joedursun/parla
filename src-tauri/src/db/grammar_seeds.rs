//! Static grammar concept seed data per language and CEFR level.
//! Inserted into the `grammar_concepts` table during onboarding.

use super::NewGrammarConcept;

/// Return grammar concepts for a given language and CEFR level.
/// Includes concepts for the target level and all lower levels.
pub fn grammar_concepts_for(language: &str, level: &str) -> Vec<NewGrammarConcept> {
    let lang = normalize_language(language);
    let levels = levels_up_to(level);
    let mut concepts = Vec::new();
    for lvl in &levels {
        let batch = match lang {
            "es" => spanish(lvl),
            "fr" => french(lvl),
            "de" => german(lvl),
            "it" => italian(lvl),
            "pt" => portuguese(lvl),
            "ja" => japanese(lvl),
            "ko" => korean(lvl),
            "zh" => chinese(lvl),
            "tr" => turkish(lvl),
            _ => spanish(lvl), // fallback
        };
        concepts.extend(batch);
    }
    concepts
}

fn normalize_language(lang: &str) -> &str {
    let lower = lang.to_lowercase();
    // Handle both codes ("es") and names ("Spanish")
    match lower.as_str() {
        "es" | "spanish" => "es",
        "fr" | "french" => "fr",
        "de" | "german" => "de",
        "it" | "italian" => "it",
        "pt" | "portuguese" => "pt",
        "ja" | "japanese" => "ja",
        "ko" | "korean" => "ko",
        "zh" | "mandarin" | "chinese" => "zh",
        "tr" | "turkish" => "tr",
        _ => "es",
    }
}

/// Return all CEFR levels up to and including the target.
fn levels_up_to(level: &str) -> Vec<&'static str> {
    let all = ["A1", "A2", "B1", "B2"];
    let target = level.split_whitespace().next().unwrap_or(level);
    let target_upper = target.to_uppercase();
    let mut result = Vec::new();
    for &l in &all {
        result.push(l);
        if l == target_upper {
            break;
        }
    }
    if result.is_empty() {
        result.push("A1");
    }
    result
}

fn c(slug: &str, name: &str, desc: &str, level: &str) -> NewGrammarConcept {
    NewGrammarConcept {
        slug: slug.to_string(),
        name: name.to_string(),
        description: desc.to_string(),
        cefr_level: level.to_string(),
    }
}

// ── Spanish ─────────────────────────────────────────────────────────────

fn spanish(level: &str) -> Vec<NewGrammarConcept> {
    match level {
        "A1" => vec![
            c("es_articles", "Articles (el/la/los/las)", "Definite and indefinite articles with gender and number", "A1"),
            c("es_noun_gender", "Noun Gender", "Masculine and feminine nouns; -o/-a patterns and exceptions", "A1"),
            c("es_ser_estar_present", "Ser vs Estar (present)", "Two 'to be' verbs: identity/traits vs state/location", "A1"),
            c("es_present_regular", "Present Tense (regular)", "Regular -ar, -er, -ir verb conjugation", "A1"),
            c("es_adjective_agreement", "Adjective Agreement", "Adjectives match noun in gender and number", "A1"),
            c("es_basic_negation", "Basic Negation", "Using 'no' before the verb", "A1"),
            c("es_questions", "Question Formation", "Interrogative words: qué, quién, dónde, cuándo, cómo", "A1"),
            c("es_hay", "Hay (there is/are)", "Expressing existence with 'hay'", "A1"),
            c("es_possessives", "Possessive Adjectives", "mi, tu, su, nuestro, vuestro, su", "A1"),
            c("es_numbers_time", "Numbers and Telling Time", "Cardinal numbers, expressing time", "A1"),
        ],
        "A2" => vec![
            c("es_present_irregular", "Present Tense (irregular)", "Common stem-changers: e→ie, o→ue, e→i", "A2"),
            c("es_preterite_regular", "Preterite (regular)", "Past actions: regular -ar, -er, -ir endings", "A2"),
            c("es_preterite_irregular", "Preterite (irregular)", "Irregular preterites: ir/ser, hacer, tener, estar", "A2"),
            c("es_reflexive_verbs", "Reflexive Verbs", "Me, te, se, nos + verb for daily routines", "A2"),
            c("es_gustar", "Gustar Construction", "'A mí me gusta' — the reverse construction for likes", "A2"),
            c("es_direct_object", "Direct Object Pronouns", "lo, la, los, las — replacing direct objects", "A2"),
            c("es_ir_a_infinitive", "Ir a + infinitive", "Near future: 'Voy a comer'", "A2"),
            c("es_comparatives", "Comparatives", "más/menos...que, tan...como, mejor/peor", "A2"),
            c("es_demonstratives", "Demonstratives", "este/ese/aquel and their forms", "A2"),
            c("es_prepositions", "Common Prepositions", "a, de, en, con, por, para basics", "A2"),
        ],
        "B1" => vec![
            c("es_imperfect", "Imperfect Tense", "Habitual past, descriptions, background actions", "B1"),
            c("es_pret_vs_imp", "Preterite vs Imperfect", "Choosing between completed vs ongoing past", "B1"),
            c("es_present_perfect", "Present Perfect", "He/has/ha + past participle for recent past", "B1"),
            c("es_subjunctive_present", "Present Subjunctive", "Subjunctive after querer que, esperar que, es importante que", "B1"),
            c("es_indirect_object", "Indirect Object Pronouns", "me, te, le, nos, les — and doubling with 'a'", "B1"),
            c("es_por_para", "Por vs Para", "Distinguishing 'for' — reason/exchange vs purpose/destination", "B1"),
            c("es_conditional", "Conditional Tense", "Would do: hablar → hablaría", "B1"),
            c("es_relative_clauses", "Relative Clauses", "que, quien, donde, lo que in relative clauses", "B1"),
            c("es_commands_informal", "Informal Commands (tú)", "Affirmative and negative tú imperatives", "B1"),
            c("es_adverbs", "Adverbs (-mente)", "Forming adverbs from adjectives + irregular adverbs", "B1"),
        ],
        "B2" => vec![
            c("es_subjunctive_past", "Past Subjunctive", "Imperfect subjunctive: si yo fuera, quisiera que", "B2"),
            c("es_conditional_si", "Si Clauses (conditionals)", "Real and unreal conditionals: si + present/past subjunctive", "B2"),
            c("es_passive_voice", "Passive Voice", "Ser + participle and 'se' passive constructions", "B2"),
            c("es_perfect_tenses", "Compound Perfect Tenses", "Pluperfect, future perfect, conditional perfect", "B2"),
            c("es_subjunctive_triggers", "Subjunctive Triggers", "WEIRDO categories: wishes, emotion, impersonal, recommendations, doubt, ojalá", "B2"),
            c("es_formal_commands", "Formal Commands (usted)", "Usted/ustedes imperatives", "B2"),
            c("es_discourse_connectors", "Discourse Connectors", "sin embargo, por lo tanto, en cambio, a pesar de", "B2"),
            c("es_nominalizations", "Nominalizations", "Using articles with adjectives/infinitives as nouns", "B2"),
        ],
        _ => vec![],
    }
}

// ── French ──────────────────────────────────────────────────────────────

fn french(level: &str) -> Vec<NewGrammarConcept> {
    match level {
        "A1" => vec![
            c("fr_articles", "Articles (le/la/les/un/une)", "Definite and indefinite articles", "A1"),
            c("fr_noun_gender", "Noun Gender", "Masculine and feminine nouns", "A1"),
            c("fr_present_regular", "Present Tense (regular)", "-er, -ir, -re verb conjugation", "A1"),
            c("fr_etre_avoir", "Être and Avoir", "Conjugation and usage of the two core verbs", "A1"),
            c("fr_negation", "Negation (ne...pas)", "Basic sentence negation", "A1"),
            c("fr_adjective_agreement", "Adjective Agreement", "Gender and number agreement, position rules", "A1"),
            c("fr_questions", "Question Formation", "Est-ce que, inversion, intonation", "A1"),
            c("fr_possessives", "Possessive Adjectives", "mon/ma/mes, ton/ta/tes, son/sa/ses", "A1"),
            c("fr_il_y_a", "Il y a", "Expressing existence: there is/are", "A1"),
        ],
        "A2" => vec![
            c("fr_passe_compose", "Passé Composé", "Past tense with avoir/être + past participle", "A2"),
            c("fr_imparfait", "Imparfait", "Ongoing/habitual past actions", "A2"),
            c("fr_reflexive", "Reflexive Verbs", "se lever, se coucher — pronominal verbs", "A2"),
            c("fr_pronouns_cod_coi", "Object Pronouns (COD/COI)", "le/la/les, lui/leur — direct and indirect", "A2"),
            c("fr_futur_proche", "Futur Proche", "aller + infinitive for near future", "A2"),
            c("fr_comparatives", "Comparatives & Superlatives", "plus/moins/aussi...que, le plus/le moins", "A2"),
            c("fr_partitive", "Partitive Articles", "du, de la, des — some/any", "A2"),
            c("fr_imperative", "Imperative", "Command forms for tu, nous, vous", "A2"),
        ],
        "B1" => vec![
            c("fr_pc_vs_imp", "Passé Composé vs Imparfait", "Choosing between completed vs background past", "B1"),
            c("fr_subjonctif", "Present Subjunctive", "After il faut que, je veux que, emotion verbs", "B1"),
            c("fr_conditionnel", "Conditional", "Would: je voudrais, j'aimerais", "B1"),
            c("fr_si_clauses", "Si Clauses", "Conditional sentences: si + imparfait → conditionnel", "B1"),
            c("fr_relative_pronouns", "Relative Pronouns", "qui, que, dont, où in relative clauses", "B1"),
            c("fr_y_en", "Pronouns Y and En", "Replacing à + noun (y) and de + noun (en)", "B1"),
            c("fr_plus_que_parfait", "Plus-que-parfait", "Pluperfect: had done", "B1"),
        ],
        "B2" => vec![
            c("fr_subjonctif_past", "Past Subjunctive", "que j'aie fait — subjunctive for past events", "B2"),
            c("fr_passive", "Passive Voice", "être + past participle constructions", "B2"),
            c("fr_futur_anterieur", "Futur Antérieur", "Future perfect: will have done", "B2"),
            c("fr_ne_expletif", "Ne Explétif", "Non-negative ne after avant que, à moins que", "B2"),
            c("fr_discourse_markers", "Discourse Markers", "cependant, néanmoins, en revanche, toutefois", "B2"),
        ],
        _ => vec![],
    }
}

// ── German ──────────────────────────────────────────────────────────────

fn german(level: &str) -> Vec<NewGrammarConcept> {
    match level {
        "A1" => vec![
            c("de_articles_cases", "Articles & Cases (Nom/Akk)", "der/die/das, ein/eine and nominative vs accusative", "A1"),
            c("de_present_regular", "Present Tense", "Regular and common irregular verbs", "A1"),
            c("de_word_order", "Word Order (V2)", "Verb-second rule in main clauses", "A1"),
            c("de_negation", "Negation (nicht/kein)", "Sentence vs noun negation", "A1"),
            c("de_modal_verbs", "Modal Verbs", "können, müssen, wollen, dürfen, sollen, mögen", "A1"),
            c("de_possessives", "Possessive Pronouns", "mein, dein, sein, ihr, unser, euer", "A1"),
            c("de_separable_verbs", "Separable Verbs", "anfangen, aufstehen — prefix goes to end", "A1"),
            c("de_plural", "Noun Plurals", "Die five plural patterns: -e, -er, -n, -en, -s, umlaut", "A1"),
        ],
        "A2" => vec![
            c("de_dative", "Dative Case", "dem/der/den — indirect objects and dative prepositions", "A2"),
            c("de_perfekt", "Perfekt (conversational past)", "haben/sein + past participle", "A2"),
            c("de_prepositions", "Prepositions with Cases", "Two-way prepositions: in, an, auf, über, unter, etc.", "A2"),
            c("de_subordinate_clauses", "Subordinate Clauses", "Verb-final with weil, dass, wenn, ob", "A2"),
            c("de_reflexive", "Reflexive Verbs", "sich waschen, sich freuen — reflexive pronouns", "A2"),
            c("de_comparatives", "Comparatives & Superlatives", "größer als, am größten", "A2"),
            c("de_imperative", "Imperative", "du/ihr/Sie command forms", "A2"),
        ],
        "B1" => vec![
            c("de_genitive", "Genitive Case", "des/der — possession and genitive prepositions", "B1"),
            c("de_praeteritum", "Präteritum (written past)", "Simple past for narratives", "B1"),
            c("de_konjunktiv_ii", "Konjunktiv II", "Subjunctive for wishes and conditionals: würde, wäre, hätte", "B1"),
            c("de_passive", "Passive Voice", "werden + past participle", "B1"),
            c("de_relative_clauses", "Relative Clauses", "der/die/das as relative pronouns", "B1"),
            c("de_infinitive_zu", "Infinitive with zu", "um...zu, ohne...zu, anstatt...zu", "B1"),
            c("de_adjective_endings", "Adjective Endings", "Strong, weak, and mixed declension", "B1"),
        ],
        "B2" => vec![
            c("de_konjunktiv_i", "Konjunktiv I", "Indirect speech: er sage, sie habe", "B2"),
            c("de_plusquamperfekt", "Plusquamperfekt", "Past perfect: hatte gemacht, war gegangen", "B2"),
            c("de_future_perfect", "Futur II", "Future perfect: wird gemacht haben", "B2"),
            c("de_extended_attributes", "Extended Attributes", "Participial attributes before nouns", "B2"),
            c("de_connectors", "Advanced Connectors", "dementsprechend, infolgedessen, nichtsdestotrotz", "B2"),
        ],
        _ => vec![],
    }
}

// ── Italian ─────────────────────────────────────────────────────────────

fn italian(level: &str) -> Vec<NewGrammarConcept> {
    match level {
        "A1" => vec![
            c("it_articles", "Articles (il/lo/la/i/gli/le)", "Definite and indefinite articles", "A1"),
            c("it_present_regular", "Present Tense (regular)", "-are, -ere, -ire verb conjugation", "A1"),
            c("it_essere_avere", "Essere and Avere", "Core verbs: to be and to have", "A1"),
            c("it_adjective_agreement", "Adjective Agreement", "Gender and number, position rules", "A1"),
            c("it_negation", "Negation", "Non before the verb", "A1"),
            c("it_questions", "Question Formation", "Che, chi, dove, quando, come, perché", "A1"),
            c("it_possessives", "Possessive Adjectives", "il mio, la tua — with articles", "A1"),
            c("it_ci_e", "C'è / Ci sono", "There is / there are", "A1"),
        ],
        "A2" => vec![
            c("it_passato_prossimo", "Passato Prossimo", "Past tense: avere/essere + past participle", "A2"),
            c("it_imperfetto", "Imperfetto", "Habitual and descriptive past", "A2"),
            c("it_reflexive", "Reflexive Verbs", "alzarsi, lavarsi — reflexive pronouns", "A2"),
            c("it_object_pronouns", "Object Pronouns", "lo/la/li/le, mi/ti/ci/vi — direct and indirect", "A2"),
            c("it_future", "Future Tense", "Regular and irregular future: andrò, farò", "A2"),
            c("it_imperative", "Imperative", "tu/noi/voi command forms", "A2"),
            c("it_prepositions", "Prepositions", "di, a, da, in, con, su + articulated forms", "A2"),
        ],
        "B1" => vec![
            c("it_pp_vs_imp", "Passato Prossimo vs Imperfetto", "Completed vs ongoing/habitual past", "B1"),
            c("it_congiuntivo", "Congiuntivo Presente", "Subjunctive after pensare che, volere che", "B1"),
            c("it_condizionale", "Condizionale", "Would: vorrei, potrei, dovrei", "B1"),
            c("it_si_impersonale", "Si Impersonale/Passivante", "Impersonal and passive si constructions", "B1"),
            c("it_relative_pronouns", "Relative Pronouns", "che, cui, il quale in relative clauses", "B1"),
            c("it_ne_ci", "Ne and Ci", "Pronominal particles for partitive and locative", "B1"),
        ],
        "B2" => vec![
            c("it_congiuntivo_past", "Congiuntivo Imperfetto/Trapassato", "Past subjunctive forms", "B2"),
            c("it_periodo_ipotetico", "Periodo Ipotetico", "If-clauses: real, possible, and impossible conditions", "B2"),
            c("it_passivo", "Passive Voice", "essere/venire + past participle", "B2"),
            c("it_gerundio", "Gerundio", "Progressive and adverbial uses of -ando/-endo", "B2"),
            c("it_discourse_markers", "Discourse Markers", "tuttavia, pertanto, nonostante, anzi", "B2"),
        ],
        _ => vec![],
    }
}

// ── Portuguese ──────────────────────────────────────────────────────────

fn portuguese(level: &str) -> Vec<NewGrammarConcept> {
    match level {
        "A1" => vec![
            c("pt_articles", "Articles (o/a/os/as)", "Definite and indefinite articles with contractions", "A1"),
            c("pt_present_regular", "Present Tense (regular)", "-ar, -er, -ir verb conjugation", "A1"),
            c("pt_ser_estar", "Ser vs Estar", "Permanent traits vs temporary states/location", "A1"),
            c("pt_adjective_agreement", "Adjective Agreement", "Gender and number agreement", "A1"),
            c("pt_negation", "Negation", "Não before the verb", "A1"),
            c("pt_questions", "Question Formation", "O que, quem, onde, quando, como, por quê", "A1"),
            c("pt_possessives", "Possessive Adjectives", "meu/minha, teu/tua, seu/sua", "A1"),
            c("pt_ter_haver", "Ter and Haver", "Possession and existence: tem/há", "A1"),
        ],
        "A2" => vec![
            c("pt_preterite", "Pretérito Perfeito", "Completed past actions", "A2"),
            c("pt_imperfect", "Pretérito Imperfeito", "Habitual/descriptive past", "A2"),
            c("pt_reflexive", "Reflexive Verbs", "se + verb for daily routines", "A2"),
            c("pt_object_pronouns", "Object Pronouns", "o/a/os/as, me/te/lhe — placement rules", "A2"),
            c("pt_future_ir", "Ir + infinitive", "Near future construction", "A2"),
            c("pt_comparatives", "Comparatives", "mais/menos...do que, tão...como", "A2"),
            c("pt_imperative", "Imperative", "Command forms for tu and você", "A2"),
        ],
        "B1" | "B2" => vec![
            c("pt_subjunctive", "Present Subjunctive", "After espero que, talvez, para que", "B1"),
            c("pt_conditional", "Conditional", "Would: falaria, faria", "B1"),
            c("pt_pluperfect", "Mais-que-perfeito", "Had done: compound and simple forms", "B1"),
            c("pt_personal_infinitive", "Personal Infinitive", "Inflected infinitive unique to Portuguese", "B1"),
            c("pt_passive", "Passive Voice", "ser + past participle", "B1"),
        ],
        _ => vec![],
    }
}

// ── Japanese ────────────────────────────────────────────────────────────

fn japanese(level: &str) -> Vec<NewGrammarConcept> {
    match level {
        "A1" => vec![
            c("ja_desu_masu", "Desu/Masu Form", "Polite sentence endings: です、ます", "A1"),
            c("ja_particles_wa_ga", "Particles は and が", "Topic marker vs subject marker", "A1"),
            c("ja_particles_wo_ni_de", "Particles を、に、で", "Object, location/time, and means particles", "A1"),
            c("ja_adjectives", "Adjective Types", "い-adjectives vs な-adjectives and conjugation", "A1"),
            c("ja_te_form", "て-Form", "Connecting verbs, making requests: 〜てください", "A1"),
            c("ja_existence", "ある and いる", "Existence for inanimate vs animate", "A1"),
            c("ja_counters", "Basic Counters", "つ、人、本、枚 — counting objects", "A1"),
            c("ja_question_ka", "Question Formation", "か particle and question words", "A1"),
        ],
        "A2" => vec![
            c("ja_past_tense", "Past Tense", "ました、でした and plain past forms", "A2"),
            c("ja_tai_form", "〜たい Form", "Expressing wants: 食べたい", "A2"),
            c("ja_te_iru", "〜ている Form", "Ongoing actions and resultant states", "A2"),
            c("ja_potential", "Potential Form", "Can do: 食べられる、読める", "A2"),
            c("ja_giving_receiving", "Giving and Receiving", "あげる、もらう、くれる", "A2"),
            c("ja_nai_form", "Negative Form", "ない conjugation and usage", "A2"),
            c("ja_particles_advanced", "Particles も、と、や、から、まで", "Additional particles for listing, from/to", "A2"),
        ],
        "B1" => vec![
            c("ja_conditional", "Conditional Forms", "〜たら、〜ば、〜と、〜なら — four conditionals", "B1"),
            c("ja_passive", "Passive Voice", "〜られる for passive and adversative passive", "B1"),
            c("ja_causative", "Causative Form", "〜させる — making/letting someone do", "B1"),
            c("ja_volitional", "Volitional Form", "〜よう — let's / intend to", "B1"),
            c("ja_relative_clauses", "Relative Clauses", "Noun modification with plain form", "B1"),
            c("ja_keigo_intro", "Keigo Introduction", "Honorific (尊敬語) and humble (謙譲語) basics", "B1"),
        ],
        "B2" => vec![
            c("ja_causative_passive", "Causative-Passive", "〜させられる — being made to do", "B2"),
            c("ja_keigo_advanced", "Advanced Keigo", "Complex honorific and humble expressions", "B2"),
            c("ja_grammar_patterns", "N2 Grammar Patterns", "〜に違いない、〜わけがない、〜ものだ", "B2"),
            c("ja_formal_writing", "Formal Writing Style", "である体、literary forms", "B2"),
        ],
        _ => vec![],
    }
}

// ── Korean ──────────────────────────────────────────────────────────────

fn korean(level: &str) -> Vec<NewGrammarConcept> {
    match level {
        "A1" => vec![
            c("ko_particles_subject", "Subject Particles 이/가, 은/는", "Subject vs topic markers", "A1"),
            c("ko_particles_object", "Object Particle 을/를", "Marking the direct object", "A1"),
            c("ko_present_tense", "Present Tense -아/어요", "Polite present tense conjugation", "A1"),
            c("ko_past_tense", "Past Tense -았/었어요", "Polite past tense", "A1"),
            c("ko_negation", "Negation 안, -지 않다", "Two ways to negate", "A1"),
            c("ko_location", "Location Particles 에, 에서", "Static location vs action location", "A1"),
            c("ko_want", "고 싶다 (want to)", "Expressing desires", "A1"),
            c("ko_numbers", "Native & Sino-Korean Numbers", "Two number systems and when to use each", "A1"),
        ],
        "A2" => vec![
            c("ko_connecting", "Connecting Sentences -고, -지만", "And, but conjunctions", "A2"),
            c("ko_can", "ㄹ/을 수 있다 (can)", "Expressing ability", "A2"),
            c("ko_honorifics_basic", "Basic Honorifics -(으)세요", "Polite requests and honorific speech", "A2"),
            c("ko_because", "Because -아/어서, -(으)니까", "Two cause-reason connectors", "A2"),
            c("ko_future", "Future Tense -(으)ㄹ 거예요", "Expressing future plans", "A2"),
            c("ko_adjective_form", "Adjective Modifier -(으)ㄴ", "Modifying nouns with adjectives", "A2"),
            c("ko_progressive", "Progressive -고 있다", "Ongoing actions", "A2"),
        ],
        "B1" => vec![
            c("ko_conditional", "Conditional -(으)면", "If-clauses", "B1"),
            c("ko_passive", "Passive Voice 이/히/리/기", "Passive verb forms", "B1"),
            c("ko_indirect_speech", "Indirect Speech -다고/라고", "Reporting what someone said", "B1"),
            c("ko_relative_clause", "Relative Clauses -(으)ㄴ/는/ㄹ", "Noun modification with verbs", "B1"),
            c("ko_honorifics_adv", "Advanced Honorifics", "드리다, 주시다, special honor vocabulary", "B1"),
            c("ko_conjunctions", "Advanced Conjunctions", "-(으)면서, -자마자, -(으)ㄹ 때", "B1"),
        ],
        "B2" => vec![
            c("ko_causative", "Causative 이/히/리/기/우", "Making someone do something", "B2"),
            c("ko_grammar_patterns", "Advanced Patterns", "-는 바람에, -는 셈이다, -(으)ㄹ 뿐만 아니라", "B2"),
            c("ko_formal_register", "Formal Register -ㅂ/습니다", "Formal speech level", "B2"),
            c("ko_written_style", "Written Style Endings", "-다, -(으)며, -(으)ㄴ 바", "B2"),
        ],
        _ => vec![],
    }
}

// ── Mandarin Chinese ────────────────────────────────────────────────────

fn chinese(level: &str) -> Vec<NewGrammarConcept> {
    match level {
        "A1" => vec![
            c("zh_word_order", "Basic Word Order (SVO)", "Subject-Verb-Object and time/place ordering", "A1"),
            c("zh_shi", "是 (shì) Sentences", "Identifying and equating with 是", "A1"),
            c("zh_negation", "Negation 不/没", "不 for present/future, 没 for past", "A1"),
            c("zh_questions_ma", "Questions with 吗 and 呢", "Yes/no and follow-up questions", "A1"),
            c("zh_measure_words", "Measure Words (个, 本, 杯)", "Classifiers between numbers and nouns", "A1"),
            c("zh_le_completed", "了 for Completed Action", "Verb + 了 for completion", "A1"),
            c("zh_adj_predicate", "Adjective Predicates with 很", "很好, 很大 — adjectives without 是", "A1"),
            c("zh_de_possession", "的 for Possession", "我的, 他的 — possessive particle", "A1"),
        ],
        "A2" => vec![
            c("zh_guo_experience", "过 for Experience", "Verb + 过 for past experience", "A2"),
            c("zh_zai_progressive", "在 for Progressive", "正在 + verb for ongoing actions", "A2"),
            c("zh_complement_result", "Result Complements", "Verb + 到/完/好/见 for outcomes", "A2"),
            c("zh_comparison", "Comparison with 比", "A 比 B + adjective pattern", "A2"),
            c("zh_ba_construction", "把 Construction", "Disposal form: 把 + object + verb", "A2"),
            c("zh_direction_complements", "Direction Complements", "来/去, 上/下, 进/出 after verbs", "A2"),
            c("zh_duration", "Duration Expressions", "Verb + time duration pattern", "A2"),
        ],
        "B1" => vec![
            c("zh_bei_passive", "被 Passive Construction", "被 + agent + verb for passive", "B1"),
            c("zh_complement_degree", "Degree Complements 得", "Verb 得 + description", "B1"),
            c("zh_lian_dou", "连...都/也 (even)", "Emphasis pattern for 'even'", "B1"),
            c("zh_shi_de", "是...的 Construction", "Emphasizing time, place, or manner of past actions", "B1"),
            c("zh_complex_complements", "Complex Complements", "Potential complements: 得了/不了", "B1"),
            c("zh_conjunctions", "Complex Conjunctions", "虽然...但是, 不但...而且, 因为...所以", "B1"),
        ],
        "B2" => vec![
            c("zh_written_style", "Written/Formal Style", "书面语 vs 口语 distinctions", "B2"),
            c("zh_chengyu", "Chéngyǔ (Four-character Idioms)", "Common idiomatic expressions", "B2"),
            c("zh_advanced_ba_bei", "Advanced 把/被 Patterns", "Complex disposal and passive constructions", "B2"),
            c("zh_discourse", "Discourse Markers", "总之, 换句话说, 由此可见", "B2"),
        ],
        _ => vec![],
    }
}

// ── Turkish ─────────────────────────────────────────────────────────────

fn turkish(level: &str) -> Vec<NewGrammarConcept> {
    match level {
        "A1" => vec![
            c("tr_vowel_harmony", "Vowel Harmony", "Front/back and rounded/unrounded vowel rules", "A1"),
            c("tr_present_tense", "Present Tense (-yor)", "Continuous present: yapıyorum", "A1"),
            c("tr_to_be", "To Be (suffixes)", "-(y)ım/sın/dır for am/is/are", "A1"),
            c("tr_negation", "Negation", "-me/-ma and değil", "A1"),
            c("tr_cases_basic", "Basic Cases (nom/acc/dat/loc)", "Nominative, accusative -ı, dative -e, locative -de", "A1"),
            c("tr_possessives", "Possessive Suffixes", "-(ı)m, -(ı)n, -(s)ı — my/your/his", "A1"),
            c("tr_var_yok", "Var/Yok", "There is / there isn't", "A1"),
            c("tr_questions", "Question Particle mı/mi", "Yes/no questions with mı/mi/mu/mü", "A1"),
        ],
        "A2" => vec![
            c("tr_past_di", "Past Tense (-dı)", "Witnessed/definite past: yaptım", "A2"),
            c("tr_past_mis", "Past Tense (-mış)", "Reported/hearsay past: yapmış", "A2"),
            c("tr_aorist", "Aorist Tense (-r)", "General/habitual present: yaparım", "A2"),
            c("tr_future", "Future Tense (-ecek)", "yapacağım — will do", "A2"),
            c("tr_ablative", "Ablative Case (-den)", "From: evden, okuldan", "A2"),
            c("tr_postpositions", "Postpositions", "için, ile, gibi — equivalents of prepositions", "A2"),
            c("tr_compound_nouns", "Compound Nouns", "Noun + noun with possessive suffix", "A2"),
        ],
        "B1" => vec![
            c("tr_relative_ki", "Relative Clauses with ki", "The connecting particle ki", "B1"),
            c("tr_participles", "Participles (-en/-an, -dık)", "Verb forms used as adjectives/nouns", "B1"),
            c("tr_conditional", "Conditional (-se/-sa)", "If-clauses: yaparsam, yapsam", "B1"),
            c("tr_passive", "Passive Voice (-ıl/-ın)", "yapılmak — to be done", "B1"),
            c("tr_causative", "Causative (-tır/-dır)", "yaptırmak — to have something done", "B1"),
            c("tr_converbs", "Converbs (-arak, -ınca, -ken)", "Adverbial verb forms", "B1"),
        ],
        "B2" => vec![
            c("tr_reported_speech", "Reported Speech", "Indirect quotation patterns", "B2"),
            c("tr_nominalizations", "Nominalizations (-me/-ma, -ış)", "Turning verbs into nouns", "B2"),
            c("tr_reflexive_reciprocal", "Reflexive & Reciprocal", "-ın for self, -ış for each other", "B2"),
            c("tr_formal_register", "Formal Register", "Literary and formal Turkish patterns", "B2"),
        ],
        _ => vec![],
    }
}
