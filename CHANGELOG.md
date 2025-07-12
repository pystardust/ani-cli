# Changelog

## v4.10.2
- Toegevoegd: Interactieve prompt voor het instellen van de MAL client ID als deze nog niet is geconfigureerd. Bij het starten van ani-cli zonder opties wordt nu gevraagd om je MAL client ID, met uitleg en automatische opslag in ~/.config/ani-cli/mal.conf.
- Toegevoegd: Nieuwe `--mal-id` optie voor deterministisch zoeken naar anime via MyAnimeList ID. Gebruikt de officiële MAL API voor betrouwbare resultaten.
- Toegevoegd: MAL API integratie met functie `search_anime_by_mal_id()` voor het ophalen van anime data.
- Bijgewerkt: Help tekst en README met documentatie voor de nieuwe MAL ID zoekfunctionaliteit.
- Bijgewerkt: Versienummer naar 4.10.2.
- Opgelost: linter-warnings voor POSIX-compatibiliteit en printf i.p.v. echo gebruikt. 