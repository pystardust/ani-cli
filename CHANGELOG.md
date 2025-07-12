# Changelog

## v4.10.8
- Toegevoegd: Nieuwe `--add-favorite <name>` optie voor het toevoegen van anime aan favorieten op basis van naam.
- Toegevoegd: Functie `add_favorite_by_name()` voor het zoeken en selecteren van anime om toe te voegen.
- Toegevoegd: Interactief menu uitgebreid met "Add to favorites" optie (optie 6).
- Toegevoegd: Automatische zoekfunctionaliteit met gebruikersselectie voor favorieten toevoegen.
- Bijgewerkt: Help tekst met documentatie en voorbeeld voor de nieuwe `--add-favorite` optie.
- Bijgewerkt: Versienummer naar 4.10.8.

## v4.10.7
- Toegevoegd: Volledig favorieten systeem voor het opslaan en beheren van favoriete anime.
- Toegevoegd: Nieuwe `--favorites` optie voor het browsen van je favoriete anime.
- Toegevoegd: Interactief menu uitgebreid met favorieten optie (optie 5).
- Toegevoegd: Favorieten functies: `add_to_favorites()`, `remove_from_favorites()`, `get_favorites()`, `is_in_favorites()`, en `show_favorites_menu()`.
- Toegevoegd: Favorieten opties in de playback loop: "add_favorite" en "remove_favorite".
- Toegevoegd: Automatische controle of anime al in favorieten staat voordat toevoegen.
- Toegevoegd: Favorieten worden opgeslagen in `~/.local/state/ani-cli/ani-favorites`.
- Bijgewerkt: Help tekst met documentatie voor de nieuwe favorieten optie.
- Bijgewerkt: Versienummer naar 4.10.7.

## v4.10.6
- Performance: Added comprehensive caching system for MAL API responses, search results, and episode lists.
- Performance: Cache expiry set to 1 hour for optimal balance between speed and freshness.
- Feature: Added --clear-cache option to manually clear cache when needed.
- Performance: Significantly faster repeated searches and MAL API calls.

## v4.10.5
- UI: Interactive menu now features color, emoji, and anime ASCII-art for a more vibrant and fun experience.

## v4.10.4
- UI: All user-facing prompts, menu, and errors are now fully in English for a consistent experience.

## v4.10.3
- Toegevoegd: Interactief menu bij het starten van ani-cli met 4 opties: normaal zoeken, zoeken op MAL ID, seizoen overview (MAL), en top anime (MAL).
- Toegevoegd: Nieuwe `--mal-season` optie voor het browsen van huidige seizoen anime via MyAnimeList API.
- Toegevoegd: Nieuwe `--mal-top` optie voor het browsen van top anime via MyAnimeList API.
- Toegevoegd: Functies `get_mal_season_anime()` en `get_mal_top_anime()` voor MAL API integratie.
- Bijgewerkt: Help tekst met documentatie voor de nieuwe seizoen en top anime opties.
- Bijgewerkt: Versienummer naar 4.10.3.
- Opgelost: Linter warnings voor subshell variabele modificaties in MAL API functies.

## v4.10.2
- Toegevoegd: Interactieve prompt voor het instellen van de MAL client ID als deze nog niet is geconfigureerd. Bij het starten van ani-cli zonder opties wordt nu gevraagd om je MAL client ID, met uitleg en automatische opslag in ~/.config/ani-cli/mal.conf.
- Toegevoegd: Nieuwe `--mal-id` optie voor deterministisch zoeken naar anime via MyAnimeList ID. Gebruikt de officiële MAL API voor betrouwbare resultaten.
- Toegevoegd: MAL API integratie met functie `search_anime_by_mal_id()` voor het ophalen van anime data.
- Bijgewerkt: Help tekst en README met documentatie voor de nieuwe MAL ID zoekfunctionaliteit.
- Bijgewerkt: Versienummer naar 4.10.2.
- Opgelost: linter-warnings voor POSIX-compatibiliteit en printf i.p.v. echo gebruikt. 